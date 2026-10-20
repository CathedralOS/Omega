//! Receiving an erased mathematical actual against an independently retained
//! carrier. Canonical decoding establishes representation only; this explicit
//! boundary operation checks the judgment and the receiver's assumption grants.

mod slots;

use std::collections::BTreeSet;

use proof_admission::{
    Budget, CoreError, MathematicalCertificate, MathematicalCertificateBatchError, TermArena,
    certificate_assumption_closure, verify_mathematical_certificate,
    verify_mathematical_certificates,
};
use semantic_vocabulary::ProofTerm;

use crate::{CodecError, decode_mathematical_certificate, encode_mathematical_certificate};

#[derive(Debug)]
pub enum MathematicalProofActualError {
    NotMathematical,
    SourceCarrierMismatch,
    KernelCarrierMismatch,
    OpenContext,
    UngrantedAssumptions(BTreeSet<u32>),
    Wire(CodecError),
    Kernel(CoreError),
    KernelBatch(MathematicalCertificateBatchError),
}

/// Canonical signature, universe/context telescope and claimed carrier. The
/// repeated carrier root makes this a representation key, not a new judgment.
/// The receiver obtains its expected key from the retained checked contract,
/// independently of the untrusted actual being checked.
pub fn mathematical_carrier_key(
    arena: &TermArena,
    certificate: &MathematicalCertificate,
) -> Result<Vec<u8>, CodecError> {
    encode_mathematical_certificate(
        arena,
        &MathematicalCertificate {
            term: certificate.expected,
            ..certificate.clone()
        },
    )
}

/// Check a closed erased actual. Neither source spelling nor a successful
/// kernel type judgment alone identifies the receiver's required carrier.
/// Grants name declaration positions under the exact signature committed by
/// `expected_carrier_key`; no producer-selected grant roster is accepted.
pub fn verify_mathematical_proof_actual(
    actual: &ProofTerm,
    expected_source_identity: &str,
    expected_carrier_key: &[u8],
    granted_assumptions: &BTreeSet<u32>,
    budget: &mut Budget,
) -> Result<(), MathematicalProofActualError> {
    let mut decoded =
        prepare_mathematical_proof_actual(actual, expected_source_identity, expected_carrier_key)?;
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, budget)
        .map_err(MathematicalProofActualError::Kernel)?;
    check_grants(&decoded, granted_assumptions)
}

fn prepare_mathematical_proof_actual(
    actual: &ProofTerm,
    expected_source_identity: &str,
    expected_carrier_key: &[u8],
) -> Result<crate::DecodedMathematicalCertificate, MathematicalProofActualError> {
    let ProofTerm::Mathematical {
        type_identity,
        certificate,
    } = actual
    else {
        return Err(MathematicalProofActualError::NotMathematical);
    };
    if type_identity != expected_source_identity {
        return Err(MathematicalProofActualError::SourceCarrierMismatch);
    }
    let decoded =
        decode_mathematical_certificate(certificate).map_err(MathematicalProofActualError::Wire)?;
    if !decoded.certificate.context.is_empty() || decoded.certificate.level_arity != 0 {
        return Err(MathematicalProofActualError::OpenContext);
    }
    if mathematical_carrier_key(&decoded.arena, &decoded.certificate)
        .map_err(MathematicalProofActualError::Wire)?
        != expected_carrier_key
    {
        return Err(MathematicalProofActualError::KernelCarrierMismatch);
    }
    Ok(decoded)
}

fn check_grants(
    decoded: &crate::DecodedMathematicalCertificate,
    granted_assumptions: &BTreeSet<u32>,
) -> Result<(), MathematicalProofActualError> {
    let closure = certificate_assumption_closure(&decoded.arena, &decoded.certificate);
    let ungranted = closure
        .difference(granted_assumptions)
        .copied()
        .collect::<BTreeSet<_>>();
    if !ungranted.is_empty() {
        return Err(MathematicalProofActualError::UngrantedAssumptions(
            ungranted,
        ));
    }
    Ok(())
}

/// Re-decide evidence at an independently trusted source transport inventory.
/// `expected` must come from independently checked source/retained contracts,
/// never from the received module. The policy authorizes expected carrier rows;
/// neither received actuals nor their destination metadata choose the question.
pub fn verify_module_mathematical_actuals(
    module: &terminal_psi::TerminalModule,
    expected: &terminal_psi::TerminalModule,
    mut grants: impl FnMut(
        &terminal_psi::TerminalMathematicalCarrier,
    ) -> Result<BTreeSet<u32>, MathematicalProofActualError>,
    budget: &mut Budget,
) -> Result<(), MathematicalProofActualError> {
    use MathematicalProofActualError as Error;
    let actual_slots = slots::collect(module)?;
    let expected_slots = slots::collect(expected)?;
    slots::bind_rosters(module, expected)?;
    if actual_slots.len() != expected_slots.len() {
        return Err(Error::SourceCarrierMismatch);
    }
    let mut pending = Vec::new();
    let mut matched = BTreeSet::new();
    for actual in &actual_slots {
        let mut candidates = expected_slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.key == actual.key);
        let (ordinal, trusted) = candidates.next().ok_or(Error::SourceCarrierMismatch)?;
        if candidates.next().is_some() || !matched.insert(ordinal) {
            return Err(Error::SourceCarrierMismatch);
        }
        slots::bind_slot(module, actual, expected, trusted)?;
        if actual.actuals.len() != trusted.formals.len() {
            return Err(Error::SourceCarrierMismatch);
        }
        for (term, formal) in actual.actuals.iter().zip(trusted.formals) {
            let carrier = expected
                .mathematical_carriers
                .iter()
                .find(|row| row.type_identity == formal.type_identity);
            match (term, carrier) {
                (ProofTerm::Mathematical { .. }, Some(carrier)) => {
                    let identity = expected
                        .identities
                        .text(formal.type_identity)
                        .ok_or(Error::SourceCarrierMismatch)?;
                    let actual_carrier = module
                        .mathematical_carriers
                        .iter()
                        .find(|row| module.identities.text(row.type_identity) == Some(identity))
                        .ok_or(Error::KernelCarrierMismatch)?;
                    if actual_carrier.carrier_key != carrier.carrier_key {
                        return Err(Error::KernelCarrierMismatch);
                    }
                    let decoded =
                        prepare_mathematical_proof_actual(term, identity, &carrier.carrier_key)?;
                    pending.push((decoded, grants(carrier)?));
                }
                (ProofTerm::Formal { position }, Some(carrier)) => {
                    let _authorized = grants(carrier)?;
                    let source = actual
                        .block
                        .erased_proof_formals
                        .get(*position as usize)
                        .ok_or(Error::SourceCarrierMismatch)?;
                    let trusted_source = trusted
                        .block
                        .erased_proof_formals
                        .get(*position as usize)
                        .ok_or(Error::SourceCarrierMismatch)?;
                    if module.identities.text(source.type_identity)
                        != expected.identities.text(formal.type_identity)
                        || expected.identities.text(trusted_source.type_identity)
                            != expected.identities.text(formal.type_identity)
                    {
                        return Err(Error::SourceCarrierMismatch);
                    }
                }
                (_, Some(_)) => return Err(Error::NotMathematical),
                (_, None)
                    if slots::contains_mathematical(
                        module,
                        term,
                        &actual.block.erased_proof_formals,
                    ) =>
                {
                    return Err(Error::KernelCarrierMismatch);
                }
                _ => {}
            }
        }
    }
    let judgments = pending
        .iter()
        .map(|(decoded, _)| (&decoded.arena, &decoded.certificate))
        .collect::<Vec<_>>();
    verify_mathematical_certificates(&judgments, budget).map_err(Error::KernelBatch)?;
    for (decoded, granted) in &pending {
        check_grants(decoded, granted)?;
    }
    Ok(())
}

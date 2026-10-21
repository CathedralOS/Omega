//! Apply a closed checked function to the receiving scope's exact proof formals.
//! The certificate never imports an open context. Each Pi domain and the final
//! result have independent source carrier keys under the same signature; the
//! existing kernel checks the closed function and the existing policy grants
//! its exact assumption closure. This adds no rule beyond ordinary Pi/Apply.

use proof_admission::{Budget, Signature, Term, weak_head_normalize};
use terminal_psi::{ErasedProofFormal, TerminalModule};

use super::{MathematicalProofActualError as Error, mathematical_carrier_key};

pub(super) fn prepare(
    module: &TerminalModule,
    caller: &[ErasedProofFormal],
    expected: &TerminalModule,
    trusted_caller: &[ErasedProofFormal],
    certificate: &[u8],
    formal_arguments: &[u32],
    expected_result_key: &[u8],
    budget: &mut Budget,
) -> Result<crate::DecodedMathematicalCertificate, Error> {
    if formal_arguments.is_empty() {
        return Err(Error::SourceCarrierMismatch);
    }
    let mut decoded = crate::decode_mathematical_certificate(certificate).map_err(Error::Wire)?;
    if decoded.certificate.level_arity != 0 || !decoded.certificate.context.is_empty() {
        return Err(Error::OpenContext);
    }
    let signature = Signature::from_declarations(decoded.certificate.signature.clone());
    let mut result = decoded.certificate.expected;
    for position in formal_arguments {
        let source = caller
            .get(*position as usize)
            .ok_or(Error::SourceCarrierMismatch)?;
        let trusted = trusted_caller
            .get(*position as usize)
            .ok_or(Error::SourceCarrierMismatch)?;
        let identity = expected
            .identities
            .text(trusted.type_identity)
            .filter(|identity| !identity.is_empty())
            .ok_or(Error::SourceCarrierMismatch)?;
        if module.identities.text(source.type_identity) != Some(identity) {
            return Err(Error::SourceCarrierMismatch);
        }
        let carrier = expected
            .mathematical_carriers
            .iter()
            .find(|row| row.type_identity == trusted.type_identity)
            .ok_or(Error::KernelCarrierMismatch)?;
        let actual_carrier = module
            .mathematical_carriers
            .iter()
            .find(|row| row.type_identity == source.type_identity)
            .ok_or(Error::KernelCarrierMismatch)?;
        if actual_carrier.carrier_key != carrier.carrier_key {
            return Err(Error::KernelCarrierMismatch);
        }
        let function = weak_head_normalize(&mut decoded.arena, &signature, result, budget)
            .map_err(Error::Kernel)?;
        let Term::Pi { domain, codomain } = decoded.arena.get(function) else {
            return Err(Error::KernelCarrierMismatch);
        };
        let key = mathematical_carrier_key(
            &decoded.arena,
            &proof_admission::MathematicalCertificate {
                term: domain,
                expected: domain,
                ..decoded.certificate.clone()
            },
        )
        .map_err(Error::Wire)?;
        // A dependent/open domain cannot equal a checked closed carrier key.
        // No domain is inferred from the untrusted actual's claimed type.
        if key != carrier.carrier_key {
            return Err(Error::KernelCarrierMismatch);
        }
        result = codomain;
    }
    let key = mathematical_carrier_key(
        &decoded.arena,
        &proof_admission::MathematicalCertificate {
            term: result,
            expected: result,
            ..decoded.certificate.clone()
        },
    )
    .map_err(Error::Wire)?;
    if key != expected_result_key {
        return Err(Error::KernelCarrierMismatch);
    }
    Ok(decoded)
}

//! Apply a closed checked function to source-bound scalar/proof arguments.
//! The certificate never imports an open context. Each Pi domain and the final
//! result have independent source carrier keys under the same signature; the
//! existing kernel checks the closed function and the existing policy grants
//! its exact assumption closure. This adds no rule beyond ordinary Pi/Apply.

use proof_admission::{Budget, Context, Signature, Term, infer_sort, weak_head_normalize};
use semantic_vocabulary::{MathematicalArgument, ScalarTerm};
use terminal_psi::{ErasedProofFormal, TerminalModule};

use super::{MathematicalProofActualError as Error, mathematical_carrier_key};

pub(super) fn prepare(
    module: &TerminalModule,
    caller: &[ErasedProofFormal],
    expected: &TerminalModule,
    trusted_caller: &[ErasedProofFormal],
    certificate: &[u8],
    arguments: &[MathematicalArgument],
    expected_result_key: &[u8],
    slot: &super::slots::Slot<'_>,
    trusted_slot: &super::slots::Slot<'_>,
    budget: &mut Budget,
) -> Result<crate::DecodedMathematicalCertificate, Error> {
    if arguments.is_empty() {
        return Err(Error::SourceCarrierMismatch);
    }
    let mut decoded = crate::decode_mathematical_certificate(certificate).map_err(Error::Wire)?;
    if decoded.certificate.level_arity != 0 || !decoded.certificate.context.is_empty() {
        return Err(Error::OpenContext);
    }
    let signature = Signature::from_declarations(decoded.certificate.signature.clone());
    let mut result = decoded.certificate.expected;
    for argument in arguments {
        let carrier = match argument {
            MathematicalArgument::Formal { position } => {
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
                carrier
            }
            MathematicalArgument::Scalar {
                type_identity,
                term,
            } => {
                // Runtime observations never become kernel constants. The
                // checked closed function is universally quantified over this
                // independently formed domain. Exact source slots bind which
                // immutable scalar supplies it; its SSA type is checked in both
                // source and receiving scopes before ordinary Pi elimination.
                let ScalarTerm::Value { id, scalar_type } = term else {
                    return Err(Error::SourceCarrierMismatch);
                };
                for scope in [slot, trusted_slot] {
                    let mut found = false;
                    for value in scope.owner.parameters.iter().chain(&scope.block.parameters) {
                        if value.id == *id {
                            if value.scalar_type != *scalar_type {
                                return Err(Error::SourceCarrierMismatch);
                            }
                            found = true;
                        }
                    }
                    if !found {
                        return Err(Error::SourceCarrierMismatch);
                    }
                }
                let carrier = expected
                    .mathematical_carriers
                    .iter()
                    .find(|row| {
                        expected.identities.text(row.type_identity) == Some(type_identity.as_str())
                    })
                    .ok_or(Error::KernelCarrierMismatch)?;
                let actual_carrier = module
                    .mathematical_carriers
                    .iter()
                    .find(|row| {
                        module.identities.text(row.type_identity) == Some(type_identity.as_str())
                    })
                    .ok_or(Error::KernelCarrierMismatch)?;
                if actual_carrier.carrier_key != carrier.carrier_key {
                    return Err(Error::KernelCarrierMismatch);
                }
                carrier
            }
        };
        let function = weak_head_normalize(&mut decoded.arena, &signature, result, budget)
            .map_err(Error::Kernel)?;
        let Term::Pi { domain, codomain } = decoded.arena.get(function) else {
            return Err(Error::KernelCarrierMismatch);
        };
        infer_sort(
            &mut decoded.arena,
            &Context::empty().with_signature(signature.clone()),
            domain,
            budget,
        )
        .map_err(Error::Kernel)?;
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
    infer_sort(
        &mut decoded.arena,
        &Context::empty().with_signature(signature.clone()),
        result,
        budget,
    )
    .map_err(Error::Kernel)?;
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

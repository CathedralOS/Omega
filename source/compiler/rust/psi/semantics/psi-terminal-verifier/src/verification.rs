use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;

use psi_core::{EvidenceIdentity, EvidenceTermId, ObligationId};
#[cfg(test)]
use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm};
use psi_proof_admission::{AcceptedFact, AdmissionProfile, EvidenceError, verify_obligation};
use psi_terminal::TerminalModule;

use crate::{
    ModuleError, ValidatedTerminalModule, VerifiedTerminalStructuralFrontiers,
    reconstruct_validated_structural_ownership_frontiers, validate_module,
};

mod affine_joins;
mod call_composition;
mod evidence_provenance;
mod float_meaning_projection;
mod integer_add_subtract;
mod integer_affine;
mod integer_conversion;
mod integer_divide_remainder;
mod integer_foundation;
mod integer_multiply;
mod integer_shift;
mod proof_bundle;
mod reconstruction;
mod substitution;
mod sufficient_reduction;

use evidence_provenance::validate_evidence_producer_provenance;
pub use float_meaning_projection::*;
use integer_foundation::*;
pub use proof_bundle::*;
use reconstruction::reconstruct_validated_terminal_obligations;
pub use reconstruction::{
    ReconstructedOperationObligation, ReconstructedTerminalObligation,
    ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet,
    reconstruct_operation_obligations, reconstruct_terminal_obligations,
};
pub(crate) use substitution::{
    substitute_proposition_structural_places, substitute_proposition_values,
};

#[cfg(test)]
use integer_add_subtract::{exact_integer_add_obligation, exact_integer_subtract_obligation};
#[cfg(test)]
use integer_affine::exact_integer_affine_interval_obligation;
use integer_affine::{
    exact_integer_affine_cast_affine_obligation, exact_integer_affine_chain_obligation,
    exact_integer_cast_chain_then_affine_suffix_obligation,
    exact_integer_cast_then_affine_chain_obligation,
    exact_integer_signed_affine_cast_affine_obligation,
    exact_integer_signed_affine_chain_obligation, exact_integer_signed_affine_initial_form,
    exact_integer_signed_affine_interval_obligation, exact_integer_signed_affine_preimage_interval,
    exact_integer_signed_affine_replay,
};
#[cfg(test)]
use integer_conversion::exact_integer_cast_obligation;
#[cfg(test)]
use integer_conversion::{
    exact_integer_affine_chain_cast_obligation, exact_integer_cast_chain_obligation,
    exact_integer_computed_prefix_cast_chain_interval_obligation,
    exact_integer_computed_prefix_cast_chain_obligation,
    exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation,
    exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation,
    exact_integer_computed_prefix_widen_chain_interval_obligation,
    exact_integer_signed_affine_chain_cast_obligation,
    exact_integer_signed_multiply_chain_cast_obligation,
};
use integer_conversion::{
    exact_integer_affine_preimage_interval, exact_integer_affine_preimage_obligation,
    exact_integer_cast_chain_root_interval, exact_integer_cast_then_offset_obligation,
    exact_integer_computed_prefix_conversion_interval_obligation,
    exact_integer_divide_remainder_cast_affine_obligation,
    exact_integer_divide_remainder_chain_hull,
    exact_integer_divide_remainder_then_affine_obligation,
    exact_integer_signed_product_interval_obligation, partial_fixed_native_integer_cast,
};
#[cfg(test)]
use integer_divide_remainder::{
    exact_integer_divide_obligation, exact_integer_remainder_obligation,
};
#[cfg(test)]
use integer_divide_remainder::{
    exact_integer_divide_obligation_with_definitions,
    exact_integer_remainder_obligation_with_definitions, saturating_integer_divide_obligation,
    saturating_integer_remainder_obligation, wrapping_integer_divide_obligation,
    wrapping_integer_remainder_obligation,
};
#[cfg(test)]
use integer_multiply::exact_integer_multiply_obligation_with_definitions;
#[cfg(test)]
use integer_multiply::{
    exact_integer_cast_chain_then_signed_product_suffix_obligation,
    exact_integer_cast_then_signed_affine_chain_obligation,
    exact_integer_cast_then_signed_multiply_chain_obligation, exact_integer_multiply_obligation,
    exact_integer_signed_multiply_chain_obligation,
};
#[cfg(test)]
use integer_shift::{
    exact_integer_affine_cast_shift_obligation,
    exact_integer_arithmetic_then_shift_chain_obligation,
    exact_integer_cast_chain_then_shift_suffix_obligation,
    exact_integer_cast_then_mixed_shift_chain_obligation,
    exact_integer_cast_then_shift_left_chain_obligation,
    exact_integer_cumulative_shift_left_obligation,
    exact_integer_divide_remainder_cast_shift_obligation,
    exact_integer_divide_remainder_then_shift_obligation,
    exact_integer_mixed_shift_chain_cast_obligation, exact_integer_mixed_shift_chain_obligation,
    exact_integer_mixed_shift_preimage, exact_integer_shift_cast_shift_obligation,
    exact_integer_shift_left_chain_obligation,
    exact_integer_shift_right_chain_cast_interval_obligation,
};
use integer_shift::{
    exact_integer_shift_cast_affine_obligation,
    exact_integer_shift_then_arithmetic_chain_obligation,
};
#[cfg(test)]
use integer_shift::{exact_integer_shift_left_obligation, exact_integer_shift_obligation};

#[cfg(test)]
use affine_joins::{
    exact_integer_affine_fork_join_obligation, exact_integer_affine_quadratic_range,
    exact_integer_distinct_root_affine_fork_join_obligation,
    exact_integer_distinct_root_affine_product_join_obligation,
    exact_integer_same_root_affine_divide_remainder_join_obligation,
    exact_integer_same_root_affine_product_join_obligation,
};

#[derive(Debug)]
pub struct VerifiedTerminalModule<'module> {
    validated: ValidatedTerminalModule<'module>,
    proof_bundle: ProofBundle,
    reconstructed_obligations: ReconstructedTerminalObligationSet,
    accepted_facts: Vec<AcceptedFact>,
    structural_frontiers: VerifiedTerminalStructuralFrontiers,
}

impl<'module> VerifiedTerminalModule<'module> {
    pub const fn module(&self) -> &'module TerminalModule {
        self.validated.module()
    }

    pub fn accepted_facts(&self) -> &[AcceptedFact] {
        &self.accepted_facts
    }

    /// Exact artifact evidence accepted for this module. Retaining the bundle
    /// lets artifact consumers re-encode the verified semantic/proof pair
    /// without consulting producer state.
    pub const fn proof_bundle(&self) -> &ProofBundle {
        &self.proof_bundle
    }

    /// The complete verifier-reconstructed proof question consumed for this
    /// result. This is retained separately from producer-selected proof routes.
    pub const fn reconstructed_obligations(&self) -> &ReconstructedTerminalObligationSet {
        &self.reconstructed_obligations
    }

    /// Exact block-, operation-, and edge-scoped custody snapshots produced by
    /// the same verifier walk that admitted this module.
    pub const fn structural_frontiers(&self) -> &VerifiedTerminalStructuralFrontiers {
        &self.structural_frontiers
    }
}

pub fn verify_module<'module>(
    module: &'module TerminalModule,
    proof_bundle: &ProofBundle,
    profile: &AdmissionProfile,
) -> Result<VerifiedTerminalModule<'module>, VerificationError> {
    let validated = validate_module(module).map_err(VerificationError::Module)?;
    let structural_frontiers = reconstruct_validated_structural_ownership_frontiers(module)
        .map_err(VerificationError::Module)?;
    validate_evidence_producer_provenance(module, proof_bundle)?;
    let reconstructed_obligations =
        reconstruct_validated_terminal_obligations(module).map_err(VerificationError::Module)?;
    let contexts = module
        .machines
        .iter()
        .map(|machine| {
            validated
                .value_context(machine)
                .map(|context| (machine.id, context))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(VerificationError::Module)?;
    let mut evidence = BTreeMap::new();
    for entry in &proof_bundle.evidence {
        if evidence
            .insert(entry.obligation, entry.route.clone())
            .is_some()
        {
            return Err(VerificationError::DuplicateEvidence(entry.obligation));
        }
    }

    let mut accepted_facts = Vec::new();
    for site in reconstructed_obligations.obligations() {
        let context = contexts
            .get(&site.owner.machine())
            .expect("validated reconstructed obligation owner exists");
        let route = evidence
            .remove(&site.obligation.id)
            .ok_or(VerificationError::MissingEvidence(site.obligation.id))?;
        let accepted = verify_obligation(
            &context,
            &site.obligation,
            &site.requirements,
            &site.semantic_axioms,
            route,
            profile,
        )
        .map_err(|error| VerificationError::RejectedEvidence {
            obligation: site.obligation.id,
            error,
        })?;
        accepted_facts.push(accepted);
    }

    if let Some(obligation) = evidence.keys().next().copied() {
        return Err(VerificationError::UnknownEvidence(obligation));
    }
    Ok(VerifiedTerminalModule {
        validated,
        proof_bundle: proof_bundle.clone(),
        reconstructed_obligations,
        accepted_facts,
        structural_frontiers,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    Module(ModuleError),
    NonDenseEvidenceProducer {
        expected: EvidenceIdentity,
        actual: EvidenceIdentity,
    },
    NonCanonicalEvidenceProducerOrder,
    DuplicateEvidenceProducerTerm(EvidenceTermId),
    UnknownEvidenceProducerTerm(EvidenceTermId),
    UnusedEvidenceProducerTerm(EvidenceTermId),
    MissingEvidenceProducer(EvidenceTermId),
    InvalidEvidenceProducer(EvidenceIdentity),
    EvidenceProducerInterfaceMismatch(EvidenceTermId),
    NonCanonicalEvidenceProducerRows(EvidenceIdentity),
    DuplicateEvidence(ObligationId),
    MissingEvidence(ObligationId),
    UnknownEvidence(ObligationId),
    RejectedEvidence {
        obligation: ObligationId,
        error: EvidenceError,
    },
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VerificationError {}

#[cfg(test)]
mod tests;

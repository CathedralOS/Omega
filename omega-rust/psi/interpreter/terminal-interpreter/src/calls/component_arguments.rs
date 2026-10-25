//! Prepare exact owned opaque arguments for a real independent child.
//!
//! Ordinary argument presentation and affine partitioning stay authoritative.
//! Only an actual unchanged host atom crosses this initial closed-Inert lane;
//! materialized backing, loans, routed claims and memberships need their own
//! transfer carriers. Preparing these values executes nothing and changes no
//! parent custody. The caller commits the staged frontier only after a real
//! installed child has accepted the tuple.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BoundaryMachineId, ClaimId, MachineId, PlaceId};
use terminal_psi::{
    BoundaryMachineDeclaration, CompletionReceipt, StructuralAccess, StructuralAffineDiscard,
    StructuralArgument, StructuralMultiplicity,
};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::program::TerminalProgram;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};

pub(super) struct PreparedComponentArguments {
    pub(super) values: Vec<TerminalStructuralValue>,
    pub(super) remaining_affine: BTreeSet<StructuralAffineDiscard>,
    pub(super) remaining_claims: BTreeMap<ClaimId, crate::execution::LiveClaim>,
    pub(super) retired_sources: Vec<PlaceId>,
}

impl<'artifact> TerminalExecution<'artifact> {
    pub(super) fn prepare_component_arguments(
        &self,
        arguments: &[StructuralArgument],
        completion_receipts: &[CompletionReceipt],
        receiver: &BoundaryMachineDeclaration,
        provider: &TerminalProgram<'artifact>,
        provider_boundary: BoundaryMachineId,
        provider_entry: MachineId,
    ) -> Result<PreparedComponentArguments, TerminalInterpretError> {
        let reject = |reason| TerminalInterpretError::UnsupportedSemanticVariant(reason);
        let supplied = provider
            .program
            .verified_boundary_machine(&provider_boundary)?;
        let entry = provider.program.verified_machine(provider_entry)?;
        if receiver.structural_parameters.len() != supplied.structural_parameters.len()
            || arguments.len() != receiver.structural_parameters.len()
            || entry.structural_parameters.len() != arguments.len()
            || !entry.content_entry_claims.is_empty()
            || entry.entry_claims.len()
                != supplied
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.multiplicity == StructuralMultiplicity::Linear)
                    .count()
        {
            return Err(malformed());
        }
        let prepared =
            self.prepare_boundary_arguments(&receiver.structural_parameters, arguments)?;
        crate::custody::bind_active_structural_arguments(
            &receiver.structural_parameters,
            &prepared.values,
            &self.program.structural_types(),
            self.structural_cases.as_map(),
            self.account.as_ref(),
        )?;
        crate::custody::validate_boundary_requirements(receiver, |index| {
            prepared.values.get(index)
        })?;
        let mut values = prepared.into_opaque_values(self.local_structural_identities.cursor())?;
        let mut remaining_affine = self.live_affine_frontier.clone();
        let receiver_types = self.program.structural_types();
        let provider_types = provider.program.structural_types();
        for (argument_index, (((argument, required), supplied), value)) in arguments
            .iter()
            .zip(&receiver.structural_parameters)
            .zip(&supplied.structural_parameters)
            .zip(&mut values)
            .enumerate()
        {
            let linear = required.multiplicity == StructuralMultiplicity::Linear;
            let receiver_type = receiver_types
                .get(&required.structural_type)
                .ok_or_else(malformed)?;
            let provider_type = provider_types
                .get(&supplied.structural_type)
                .ok_or_else(malformed)?;
            if argument.access != StructuralAccess::Owned
                || required.access != StructuralAccess::Owned
                || supplied.access != StructuralAccess::Owned
                || required.multiplicity != supplied.multiplicity
                || !required.qualifications.is_empty()
                || !supplied.qualifications.is_empty()
                || !required.projected_qualifications.is_empty()
                || !supplied.projected_qualifications.is_empty()
                || !value.qualifications.is_empty()
                || !value.projected_qualifications.is_empty()
                || receiver_type.owns_linear_claim != linear
                || provider_type.owns_linear_claim != linear
                || (!linear
                    && (!receiver_type.no_code_affine_cleanup
                        || !provider_type.no_code_affine_cleanup))
                || !self
                    .program
                    .catalogs
                    .identities
                    .text(receiver_type.identity)
                    .zip(
                        provider
                            .program
                            .catalogs
                            .identities
                            .text(provider_type.identity),
                    )
                    .is_some_and(|(left, right)| !left.is_empty() && left == right)
                || self.live_claims.values().any(|claim| {
                    claim.place == Some(argument.place)
                        && terminal_semantics::structural_paths_may_overlap(
                            &claim.path,
                            &argument.path,
                        )
                })
                || crate::custody::has_live_loan(self.reference_referents.as_map(), value)
            {
                return Err(reject(
                    "component input lacks exact owned closed Inert type/custody",
                ));
            }
            let formal = &entry.structural_parameters[argument_index];
            if formal.structural_type != supplied.structural_type
                || formal.multiplicity != supplied.multiplicity
                || formal.access != supplied.access
            {
                return Err(reject(
                    "component entry differs from its admitted structural contract",
                ));
            }
            let mut entry_claims = entry
                .entry_claims
                .iter()
                .filter(|claim| claim.input == formal.place);
            let argument_index = u32::try_from(argument_index).map_err(|_| malformed())?;
            let mut receipts = completion_receipts
                .iter()
                .filter(|receipt| receipt.argument_index == argument_index);
            if linear {
                let claim = entry_claims
                    .next()
                    .ok_or_else(|| reject("linear component input lacks its whole entry claim"))?;
                let receipt = receipts.next().ok_or_else(|| {
                    reject("linear component input lacks its actual caller claim")
                })?;
                let live = self
                    .live_claims
                    .get(&receipt.claim)
                    .ok_or_else(|| reject("linear component input claim is not live"))?;
                if !claim.path.is_empty()
                    || !receipt.path.is_empty()
                    || entry_claims.next().is_some()
                    || receipts.next().is_some()
                    || live.place != Some(argument.place)
                    || live.path != argument.path
                    || live.multiplicity != Some(StructuralMultiplicity::Linear)
                {
                    return Err(reject(
                        "linear component input lacks exactly one whole owned claim",
                    ));
                }
            } else if entry_claims.next().is_some() || receipts.next().is_some() {
                return Err(reject(
                    "nonlinear component input carries unexpected claims",
                ));
            }
            let home = StructuralRuntimePlace::from(&*value);
            let within_value = |stored: &StructuralRuntimePlace| {
                stored.opaque_identity == home.opaque_identity
                    && stored.path.starts_with(&home.path)
            };
            // Ancestor cases were authenticated by ordinary projection and
            // remain with the residual parent. Only metadata belonging to this
            // exact subtree would need to accompany the moved value.
            if self.structural_cases.keys().any(within_value)
                || self.reference_referents.keys().any(within_value)
                || self.structural_element_view_fields.keys().any(within_value)
                || self.structural_byte_storage.keys().any(within_value)
                || self.structural_primitive_storage.keys().any(within_value)
                || self
                    .structural_scalar_fields
                    .keys()
                    .any(|field| within_value(&field.parent))
                || self
                    .structural_byte_sequence_fields
                    .keys()
                    .any(|field| within_value(&field.parent))
            {
                return Err(reject(
                    "component opaque input retains unsupported subtree storage or case metadata",
                ));
            }
            // The existing storage map authenticates the actual atom. A plain
            // opaque_identity, equal layout or synthesized input cannot answer it.
            if !self.forward_host_atom(value) {
                return Err(reject(
                    "component input requires actual unchanged opaque host storage",
                ));
            }
            value.structural_type = supplied.structural_type;
            if required.multiplicity == StructuralMultiplicity::Affine {
                self.consume_affine_call_argument(&mut remaining_affine, argument)?;
            }
        }
        let retired_sources = super::frames::transferred_call_sources(
            arguments,
            &receiver.structural_parameters,
            &self.structural_values,
        )?;
        let remaining_claims = crate::custody::complete_claims(
            &self.live_claims,
            arguments,
            completion_receipts,
            &receiver.structural_parameters,
        )?;
        Ok(PreparedComponentArguments {
            values,
            remaining_affine,
            remaining_claims,
            retired_sources,
        })
    }
}

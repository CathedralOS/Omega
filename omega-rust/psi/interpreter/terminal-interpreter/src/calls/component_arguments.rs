//! Prepare exact owned opaque arguments for a real independent child.
//!
//! Ordinary argument presentation and affine partitioning stay authoritative.
//! An owned affine atom moves. An owned copyable atom crosses as another
//! occurrence and the caller keeps its own. Linear debt follows the ordinary
//! exact claim-transfer join. Materialized backing, loans and memberships
//! need their own transfer carriers.
//! Preparing these values executes nothing. The caller commits the staged
//! frontier only after a real installed child has accepted the tuple.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BoundaryMachineId, ClaimId, MachineId, PlaceId};
use terminal_psi::{
    BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, StructuralAccess,
    StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::program::TerminalProgram;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};

pub(super) struct PreparedComponentArguments {
    pub(super) values: Vec<TerminalStructuralValue>,
    pub(super) remaining_affine: BTreeSet<StructuralAffineDiscard>,
    pub(super) retired_sources: Vec<PlaceId>,
    pub(super) remaining_claims: BTreeMap<ClaimId, crate::execution::LiveClaim>,
    pub(super) borrowed_storage: crate::instance::ComponentBorrowedStorage,
}

impl<'artifact> TerminalExecution<'artifact> {
    pub(super) fn prepare_component_arguments(
        &self,
        arguments: &[StructuralArgument],
        receiver: &BoundaryMachineDeclaration,
        provider: &TerminalProgram<'artifact>,
        provider_boundary: BoundaryMachineId,
        provider_entry: MachineId,
        selected: &terminal_installation::AdmittedComponentProvider,
        completion_receipts: &[CompletionReceipt],
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
        let mut values = if receiver
            .structural_parameters
            .iter()
            .all(|parameter| parameter.access == StructuralAccess::Owned)
        {
            prepared.into_opaque_values(self.local_structural_identities.cursor())?
        } else {
            let carried = prepared.into_component_call_arguments(
                &receiver.structural_parameters,
                &self.program.structural_types(),
                self.structural_cases.as_map(),
                self.account.as_ref(),
                self.local_structural_identities.cursor(),
            )?;
            if !carried.byte_sequences.is_empty()
                || !carried.primitive_loans.is_empty()
                || !carried.element_views.is_empty()
                || !carried.scalar_arrays.is_empty()
                || carried.aggregate_backings.iter().any(|(place, backing)| {
                    backing.placed.is_some()
                        || backing.region.is_some()
                        || backing.byte_region.is_some()
                        || carried.values.get(place) != Some(&backing.value)
                })
            {
                return Err(reject(
                    "component view retains unsupported primitive, collection or placed backing",
                ));
            }
            // An identity backing is folded into the same original home in
            // the private loan below. A different backing cannot be discarded.
            receiver
                .structural_parameters
                .iter()
                .map(|parameter| {
                    carried
                        .values
                        .get(&parameter.place)
                        .cloned()
                        .ok_or_else(malformed)
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut borrowed_storage =
            crate::instance::ComponentBorrowedStorage::new(selected, self.account.as_ref());
        let mut remaining_affine = self.live_affine_frontier.clone();
        let receiver_types = self.program.structural_types();
        let provider_types = provider.program.structural_types();
        for (ordinal, (((argument, required), supplied), value)) in arguments
            .iter()
            .zip(&receiver.structural_parameters)
            .zip(&supplied.structural_parameters)
            .zip(&mut values)
            .enumerate()
        {
            let formal = &entry.structural_parameters[ordinal];
            if formal.structural_type != supplied.structural_type
                || formal.multiplicity != supplied.multiplicity
                || formal.access != supplied.access
            {
                return Err(reject(
                    "component entry differs from its admitted structural contract",
                ));
            }
            let receiver_type = receiver_types
                .get(&required.structural_type)
                .ok_or_else(malformed)?;
            let provider_type = provider_types
                .get(&supplied.structural_type)
                .ok_or_else(malformed)?;
            if argument.access == StructuralAccess::SharedBorrow
                && required.access == StructuralAccess::SharedBorrow
                && supplied.access == StructuralAccess::SharedBorrow
                && required.multiplicity == StructuralMultiplicity::Unrestricted
                && supplied.multiplicity == required.multiplicity
            {
                let presentation = selected.input_presentation(ordinal).ok_or_else(malformed)?;
                if presentation.source() != required.structural_type
                    || presentation.destination() != supplied.structural_type
                    || value.structural_type != required.structural_type
                {
                    return Err(malformed());
                }
                borrowed_storage.capture(self, value, presentation.clone())?;
                value.structural_type = supplied.structural_type;
                selected
                    .present_input_memberships(&mut value.qualifications)
                    .map_err(reject)?;
                for qualification in &mut value.projected_qualifications {
                    presentation
                        .present_qualification_path(&mut qualification.path)
                        .map_err(reject)?;
                    selected
                        .present_input_memberships(std::slice::from_mut(&mut qualification.domain))
                        .map_err(reject)?;
                }
                value.qualifications.sort();
                value.projected_qualifications.sort();
                continue;
            }
            if argument.access != StructuralAccess::Owned
                || required.access != StructuralAccess::Owned
                || supplied.access != StructuralAccess::Owned
                || required.multiplicity != supplied.multiplicity
                || !matches!(
                    required.multiplicity,
                    StructuralMultiplicity::Affine
                        | StructuralMultiplicity::Unrestricted
                        | StructuralMultiplicity::Linear
                )
                || !required.projected_qualifications.is_empty()
                || !supplied.projected_qualifications.is_empty()
                || !value.projected_qualifications.is_empty()
                || receiver_type.owns_linear_claim
                    != (required.multiplicity == StructuralMultiplicity::Linear)
                || provider_type.owns_linear_claim != receiver_type.owns_linear_claim
                || (required.multiplicity != StructuralMultiplicity::Linear
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
                || crate::custody::has_live_loan(self.reference_referents.as_map(), value)
            {
                return Err(reject(
                    "component input lacks exact owned closed Inert type/custody",
                ));
            }
            let mut entry_claims = entry
                .entry_claims
                .iter()
                .filter(|claim| claim.input == formal.place);
            let argument_index = u32::try_from(ordinal).map_err(|_| malformed())?;
            let mut receipts = completion_receipts
                .iter()
                .filter(|receipt| receipt.argument_index == argument_index);
            let carried_claim = if required.multiplicity == StructuralMultiplicity::Linear {
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
                Some(receipt.claim)
            } else if entry_claims.next().is_some() || receipts.next().is_some() {
                return Err(reject(
                    "nonlinear component input carries unexpected claims",
                ));
            } else {
                None
            };
            if self.live_claims.iter().any(|(identity, claim)| {
                claim.place == Some(argument.place)
                    && terminal_semantics::structural_paths_may_overlap(&claim.path, &argument.path)
                    && Some(*identity) != carried_claim
            }) {
                return Err(reject(
                    "component input overlaps another live ownership claim",
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
            selected
                .present_input_memberships(&mut value.qualifications)
                .map_err(reject)?;
            value.qualifications.sort();
            if required.multiplicity == StructuralMultiplicity::Affine {
                self.consume_affine_call_argument(&mut remaining_affine, argument)?;
            }
        }
        let retired_sources = super::frames::transferred_call_sources(
            arguments,
            &receiver.structural_parameters,
            &self.structural_values,
        )?;
        let entry = provider.program.verified_machine(provider_entry)?;
        let callee_values = crate::custody::bind_structural_arguments(
            &entry.structural_parameters,
            &values,
            self.account.as_ref(),
        )?;
        let transfers = completion_receipts
            .iter()
            .map(|receipt| ClaimTransfer {
                claim: receipt.claim,
                argument_index: receipt.argument_index,
                path: receipt.path.clone(),
            })
            .collect::<Vec<_>>();
        let (remaining_claims, _) = crate::custody::transfer_claims(
            &provider_types,
            &BTreeMap::new(),
            &self.live_claims,
            arguments,
            &transfers,
            &entry.structural_parameters,
            &entry.entry_claims,
            &entry.content_entry_claims,
            &callee_values,
        )?;
        Ok(PreparedComponentArguments {
            values,
            remaining_affine,
            retired_sources,
            remaining_claims,
            borrowed_storage,
        })
    }
}

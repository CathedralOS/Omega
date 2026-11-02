//! Prepare exact opaque inputs for a real independent child.
//!
//! Ordinary argument presentation and affine partitioning stay authoritative.
//! Owned initialized data uses the ordinary complete-content child initializer;
//! unchanged host atoms retain their tokens. A borrowed atom retains its real
//! paused lender. Borrowed materialized backing, reference descriptors, routed
//! claims and projected memberships need their own carriers. Whole memberships
//! use the sealed
//! domain-theory relation, never a provider annotation or a copied dense ID.
//! Preparing these values executes nothing and changes no
//! parent custody. The caller commits the staged frontier only after a real
//! installed child has accepted the tuple.

use crate::custody::validate_boundary_requirements;
use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, ClaimId, MachineId, PlaceId};
use terminal_psi::{
    BoundaryMachineDeclaration, CompletionReceipt, StructuralAccess, StructuralAffineDiscard,
    StructuralArgument, StructuralMultiplicity,
};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::program::TerminalProgram;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};

pub(super) struct PreparedComponentArguments {
    pub(super) custody: ComponentInputCustody,
    pub(super) remaining_affine: BTreeSet<StructuralAffineDiscard>,
    pub(super) remaining_claims: BTreeMap<ClaimId, crate::execution::LiveClaim>,
    pub(super) retired_sources: Vec<PlaceId>,
}

/// The original prepared tuple stays owned by the paused caller's component
/// until its real child returns and the caller acknowledges completion. The
/// mapped values are presentations for that exact child, not new backing.
pub(crate) struct ComponentInputCustody {
    pub(crate) source: Option<crate::effects::byte_buffers::BoundaryArguments>,
    pub(crate) values: Vec<TerminalStructuralValue>,
    pub(crate) contents: Vec<crate::TerminalStructuralContents>,
}

impl ComponentInputCustody {
    /// Public borrowed entry annotations establish no opaque host backing. This
    /// private carrier comes from the actual parent's authenticated host cells,
    /// retained beside the child until acknowledgement. Install that association
    /// at the child's resident homes so another checked component call can lend
    /// the same object, without converting arbitrary entry identities to backing.
    pub(crate) fn establish_lent_inputs(
        &self,
        child: &mut TerminalExecution<'_>,
    ) -> Result<(), TerminalInterpretError> {
        if self.source.is_none() {
            return Ok(());
        }
        let entry = child.program.verified_machine(child.current_machine)?;
        if entry.structural_parameters.len() != self.values.len() {
            return Err(malformed());
        }
        for (parameter, value) in entry.structural_parameters.iter().zip(&self.values) {
            if matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            ) {
                let home = StructuralRuntimePlace::from(child.structural_value(parameter.place)?);
                child.component_lent_host_atoms.insert(home.clone(), ())?;
                child.structural_host_atoms.insert(
                    home,
                    crate::structural_storage::HostStructuralPayload::Opaque(value.opaque_identity),
                )?;
            }
        }
        Ok(())
    }
}

/// A pending input cannot migrate to another caller activation or call frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ComponentCallScope {
    pub(crate) namespace: Option<u64>,
    machine: MachineId,
    block: BlockId,
    depth: usize,
}

impl<'artifact> TerminalExecution<'artifact> {
    pub(crate) fn component_call_scope(&self) -> ComponentCallScope {
        ComponentCallScope {
            namespace: self.local_structural_identities.exposure_namespace,
            machine: self.current_machine,
            block: self.current,
            depth: self.call_stack.len(),
        }
    }

    pub(super) fn prepare_component_arguments(
        &self,
        arguments: &[StructuralArgument],
        completion_receipts: &[CompletionReceipt],
        receiver: &BoundaryMachineDeclaration,
        provider: &TerminalProgram<'artifact>,
        selected: &terminal_installation::AdmittedComponentProvider,
    ) -> Result<PreparedComponentArguments, TerminalInterpretError> {
        let reject = |reason| TerminalInterpretError::UnsupportedSemanticVariant(reason);
        let supplied = provider
            .program
            .verified_boundary_machine(&selected.provider_boundary())?;
        let entry = provider.program.verified_machine(selected.candidate())?;
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
            &self.structural_types(),
            self.structural_cases.as_map(),
            self.account.as_ref(),
        )?;
        validate_boundary_requirements(receiver, |index| prepared.values.get(index))?;
        prepared.opaque_values(self.local_structural_identities.cursor())?;
        let mut prepared = prepared;
        let lending = arguments
            .iter()
            .any(|argument| argument.access != StructuralAccess::Owned);
        let mut values = if lending {
            prepared.values.clone()
        } else {
            // An entirely owned tuple has no lender to retain. Preserve its
            // existing move into the child instead of retaining a second copy.
            std::mem::take(&mut prepared.values)
        };
        let mut contents = Vec::new();
        let mut remaining_affine = self.live_affine_frontier.clone();
        let receiver_types = self.structural_types();
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
            let owned = argument.access == StructuralAccess::Owned;
            let lent = matches!(
                argument.access,
                terminal_psi::reading_borrow_pair_pattern!()
            );
            if argument.access != required.access
                || required.access != supplied.access
                || (!owned && !lent)
                || required.multiplicity != supplied.multiplicity
                || !required.projected_qualifications.is_empty()
                || !supplied.projected_qualifications.is_empty()
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
                || (owned
                    && crate::custody::has_live_loan(self.reference_referents.as_map(), value))
            {
                return Err(reject(
                    "component input lacks exact owned closed Inert type/custody",
                ));
            }
            if lent {
                // Use the existing access/availability judgment on the actual
                // caller carrier. Lending does not consume its ownership or
                // close its parent loan; the complete caller stays paused.
                self.validate_descriptor_borrow(argument, required.access)?;
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
            let receipt_index = u32::try_from(argument_index).map_err(|_| malformed())?;
            let mut receipts = completion_receipts
                .iter()
                .filter(|receipt| receipt.argument_index == receipt_index);
            let carried_claim = if linear {
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
            if lent
                && (self.structural_cases.keys().any(within_value)
                    || self.structural_byte_storage.keys().any(within_value)
                    || self.empty_array_storage.rows.keys().any(within_value)
                    || self.structural_primitive_storage.keys().any(within_value)
                    || self
                        .structural_scalar_fields
                        .keys()
                        .any(|field| within_value(&field.parent)))
            {
                return Err(reject(
                    "component borrowed input needs its original materialized storage carrier",
                ));
            }
            if self.reference_referents.keys().any(within_value)
                || self.structural_element_view_fields.keys().any(within_value)
                || self.original_storage_roots.keys().any(within_value)
                || self.provider_storage.descriptors.keys().any(within_value)
                || self
                    .provider_storage
                    .completed_profiles
                    .keys()
                    .any(within_value)
                || self
                    .activation_storage
                    .claim_descriptors
                    .keys()
                    .any(within_value)
                || self.structural_byte_descriptors.keys().any(within_value)
                || self
                    .structural_byte_sequence_fields
                    .keys()
                    .any(|field| within_value(&field.parent))
            {
                return Err(reject(
                    "component input requires a loan or resource storage transfer carrier",
                ));
            }
            // The existing storage map authenticates the actual atom. A plain
            // opaque_identity, equal layout or synthesized input cannot answer it.
            if !self.forward_host_atom(value) {
                if !owned {
                    return Err(reject(
                        "component input requires actual unchanged opaque host storage",
                    ));
                }
                contents.push(self.capture_component_contents(
                    argument,
                    value,
                    argument_index,
                    provider,
                    supplied.structural_type,
                )?);
            }
            // The caller binder authenticated these exact occurrence facts
            // before custody staging. Installation joined their full nominal
            // theories once across both artifacts. Unknown extra memberships
            // cannot be erased, and child entry binding still checks its own
            // obligations before the parent commits any consumption.
            value.qualifications = value
                .qualifications
                .iter()
                .map(|domain| {
                    selected
                        .input_membership(argument_index, *domain)
                        .ok_or_else(|| {
                            reject("component input membership has no admitted destination theory")
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            value.qualifications.sort();
            value.structural_type = supplied.structural_type;
            if owned && required.multiplicity == StructuralMultiplicity::Affine {
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
            custody: ComponentInputCustody {
                source: lending.then_some(prepared),
                values,
                contents,
            },
            remaining_affine,
            remaining_claims,
            retired_sources,
        })
    }
}

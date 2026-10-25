//! Prepare exact owned opaque arguments for a real independent child.
//!
//! Ordinary argument presentation and affine partitioning stay authoritative.
//! An owned affine atom moves. An owned copyable atom crosses as another
//! occurrence and the caller keeps its own. Linear debt, materialized backing,
//! loans, routed claims and memberships need their own transfer carriers.
//! Preparing these values executes nothing. The caller commits the staged
//! frontier only after a real installed child has accepted the tuple.

use std::collections::BTreeSet;

use semantic_vocabulary::{BoundaryMachineId, PlaceId};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralMultiplicity,
};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::program::TerminalProgram;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};

pub(super) struct PreparedComponentArguments {
    pub(super) values: Vec<TerminalStructuralValue>,
    pub(super) remaining_affine: BTreeSet<StructuralAffineDiscard>,
    pub(super) retired_sources: Vec<PlaceId>,
}

impl<'artifact> TerminalExecution<'artifact> {
    pub(super) fn prepare_component_arguments(
        &self,
        arguments: &[StructuralArgument],
        receiver: &BoundaryMachineDeclaration,
        provider: &TerminalProgram<'artifact>,
        provider_boundary: BoundaryMachineId,
    ) -> Result<PreparedComponentArguments, TerminalInterpretError> {
        let reject = |reason| TerminalInterpretError::UnsupportedSemanticVariant(reason);
        let supplied = provider
            .program
            .verified_boundary_machine(&provider_boundary)?;
        if receiver.structural_parameters.len() != supplied.structural_parameters.len()
            || arguments.len() != receiver.structural_parameters.len()
        {
            return Err(malformed());
        }
        let prepared =
            self.prepare_boundary_arguments(&receiver.structural_parameters, arguments)?;
        crate::custody::bind_active_structural_arguments(
            &receiver.structural_parameters,
            &prepared.values,
            &self.program.structural_types(),
            &self.structural_cases,
        )?;
        crate::custody::validate_boundary_requirements(receiver, |index| {
            prepared.values.get(index)
        })?;
        let mut values = prepared.into_opaque_values(self.local_structural_identities.cursor())?;
        let mut remaining_affine = self.live_affine_frontier.clone();
        let receiver_types = self.program.structural_types();
        let provider_types = provider.program.structural_types();
        for (((argument, required), supplied), value) in arguments
            .iter()
            .zip(&receiver.structural_parameters)
            .zip(&supplied.structural_parameters)
            .zip(&mut values)
        {
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
                || !matches!(
                    required.multiplicity,
                    StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                )
                || !required.qualifications.is_empty()
                || !supplied.qualifications.is_empty()
                || !required.projected_qualifications.is_empty()
                || !supplied.projected_qualifications.is_empty()
                || !value.qualifications.is_empty()
                || !value.projected_qualifications.is_empty()
                || receiver_type.owns_linear_claim
                || provider_type.owns_linear_claim
                || !receiver_type.no_code_affine_cleanup
                || !provider_type.no_code_affine_cleanup
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
                || crate::custody::has_live_loan(&self.reference_referents, value)
            {
                return Err(reject(
                    "component input lacks exact owned closed Inert type/custody",
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
        Ok(PreparedComponentArguments {
            values,
            remaining_affine,
            retired_sources,
        })
    }
}

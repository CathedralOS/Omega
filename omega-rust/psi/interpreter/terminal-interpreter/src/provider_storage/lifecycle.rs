//! Execute placement completion against the original owned inputs.
//!
//! Rejection is a custody-preserving constructor, not a host error or a fresh
//! extent. Its result claims receive new caller-local names only after the
//! boundary's exact completion receipts account for every original claim.
//! Case tags, payload cells, memberships and affine debt are staged together;
//! no input is retired while a later payload or claim check can still fail.
//! Successful initialization commits the prepared image and original whole
//! value together. The resident arena retains relative custody, never frame
//! claim identifiers. A moved extent carries that arena handle with its exact
//! allocation window; selecting Ready alone cannot establish this association.

use super::initialization::{InitializationRejection, PreparedProviderImage};
use super::{ProviderStorageSlot, ResidentStorageSlot};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::record::storage::{ConstructorStorage, ConstructorValue};
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BoundaryMachineDeclaration, Operation, OperationKind, StructuralAccess,
    StructuralAffineDiscard, StructuralOperationResult, StructuralPathSegment,
    StructuralResultClaimBinding, TerminalPlacementLifecyclePlan,
};
use terminal_semantics::{PlacementResultContract, placement_result_contract};

impl TerminalExecution<'_> {
    pub(crate) fn complete_placement_initialization(
        &mut self,
        operation: &Operation,
        boundary: &BoundaryMachineDeclaration,
        plan: &TerminalPlacementLifecyclePlan,
        prepared: PreparedProviderImage,
    ) -> Result<(), TerminalInterpretError> {
        let (vacant, resident_domain) = plan
            .storage
            .owned_content()
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
        let OperationKind::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        let [extent, original, _profile] = structural_arguments.as_slice() else {
            return Err(malformed());
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        if self.result_place_in_use(result.place)
            || !result.qualification_establishments.is_empty()
            || !self
                .loan_completions_for_operation(operation.id)?
                .is_empty()
            || prepared.binding.resident != arena::Handle::default()
        {
            return Err(malformed());
        }
        let program = std::sync::Arc::clone(&self.program);
        let PlacementResultContract::Initialize {
            result: declaration,
            ready,
            ..
        } = placement_result_contract(&program.catalogs, plan)
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?
        else {
            return Err(malformed());
        };
        if result.structural_type != declaration.structural_type
            || result.multiplicity != declaration.multiplicity
            || result.qualifications != declaration.qualifications
            || result.projected_qualifications != declaration.projected_qualifications
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "placement initialization result differs from its boundary contract",
            ));
        }
        let [field] = ready.fields.as_slice() else {
            return Err(malformed());
        };
        let [extent_parameter, value_parameter, _] = boundary.structural_parameters.as_slice()
        else {
            return Err(malformed());
        };
        let mut identity_cursor = self.local_structural_identities.cursor();
        // Both snapshots validate the complete live partition before receipts
        // retire the original caller-local claims. Only T's snapshot becomes
        // resident custody; the descriptor itself travels in the ordinary result.
        let mut retained_extent = self.stage_owned_value(
            extent,
            plan.storage.carrier,
            extent_parameter.multiplicity,
            &mut identity_cursor,
        )?;
        let custody = self.stage_owned_value(
            original,
            plan.schema,
            value_parameter.multiplicity,
            &mut identity_cursor,
        )?;
        // Stored loans must remain visible to the ordinary alias/loan owner.
        // Until resident loan forwarding exists, private retention is not a
        // substitute: it would hide the borrow when source cells are retired.
        if custody.storage.retains_references() {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "resident custody requires live forwarding of carried references",
            ));
        }
        if retained_extent.claim_paths.as_slice() != [Vec::<StructuralPathSegment>::new()]
            || !retained_extent
                .value
                .qualifications
                .contains(&plan.storage.granted)
            || !retained_extent.value.qualifications.contains(&vacant)
            || retained_extent
                .value
                .qualifications
                .contains(&resident_domain)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let expected_receipts = retained_extent.claim_paths.len() + custody.claim_paths.len();
        if completion_receipts.len() != expected_receipts {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
        let mut claims = crate::custody::complete_claims(
            &self.live_claims,
            structural_arguments,
            completion_receipts,
            &boundary.structural_parameters,
        )?;
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        for (source, parameter) in [extent, original]
            .into_iter()
            .zip([extent_parameter, value_parameter])
        {
            let transfers = self.stage_constructor_operand(source, &mut frontier, &mut consumed)?;
            if parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                && !transfers
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
        let mut value = TerminalStructuralValue::minted(
            self.local_structural_identities
                .allocate_staged(&mut identity_cursor)?,
            result.structural_type,
            Vec::new(),
        );
        let destination_path = vec![
            StructuralPathSegment::Case(ready.id),
            StructuralPathSegment::Field(field.identity),
        ];
        let destination = StructuralRuntimePlace {
            opaque_identity: value.opaque_identity,
            path: vec![StructuralPathSegment::Field(field.identity)],
        };
        let mut storage = ConstructorStorage::default();
        storage.stage_case(
            StructuralRuntimePlace::from(&value),
            result.structural_type,
            ready.id,
        );
        let child = ConstructorValue::resolve(self, extent)?;
        storage.stage(self, extent, &child, &destination, true)?;
        // Remove the old descriptor from the staged constructor. Its replacement
        // is installed only with the newly committed resident handle below.
        storage.take_provider_descriptor(&destination, prepared.binding)?;
        retained_extent
            .value
            .qualifications
            .retain(|domain| *domain != vacant);
        retained_extent.value.qualifications.push(resident_domain);
        retained_extent.value.qualifications.sort();
        terminal_psi::StructuralPathQualification::append_under(
            &retained_extent.value.qualifications,
            &retained_extent.value.projected_qualifications,
            &destination_path,
            &mut value.projected_qualifications,
        );
        storage.validate_projected_memberships(self, &value, &result.projected_qualifications)?;
        let selected = |region: &StructuralAffineDiscard| {
            storage.case_at_projection(self, &value, &region.path)
        };
        let mut received = false;
        for binding in &result.claims {
            if !self.constructor_claim_is_active(result, &binding.path, &selected)? {
                continue;
            }
            if received || binding.path != destination_path {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            received = true;
            crate::custody::bind_result_claim(
                &mut claims,
                binding,
                result.place,
                result.multiplicity,
            )?;
        }
        if !received {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        self.stage_result_affine_frontier(result, &mut frontier, &selected)?;
        let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
        let storage = storage.into_detached()?;
        let start = usize::try_from(prepared.binding.offset).map_err(|_| malformed())?;
        let end = start
            .checked_add(prepared.image.len())
            .ok_or_else(malformed)?;
        let ProviderStorageSlot::Live { allocation, .. } = self
            .provider_storage
            .allocations
            .get_mut(prepared.binding.allocation)
        else {
            return Err(malformed());
        };
        let bytes = allocation.bytes.get_mut(start..end).ok_or_else(malformed)?;

        // All refusing work is over. No user code runs in this commit and none
        // of its operations can fail after the first write. Write only the image
        // footprint: the exact original window and unused tail remain owned.
        bytes.copy_from_slice(&prepared.image);
        let resident = self
            .provider_storage
            .residents
            .insert(ResidentStorageSlot::Live {
                window: prepared.binding,
                initialized_by: plan.boundary,
                custody,
                compatibility: prepared.compatibility,
            });
        retirement.commit(self);
        storage.publish(self);
        self.provider_storage.descriptors.insert(
            destination,
            super::ProviderStorageBinding {
                resident,
                ..prepared.binding
            },
        );
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        self.structural_values.insert(result.place, value)?;
        Ok(())
    }

    pub(crate) fn reject_placement_initialization(
        &mut self,
        operation: &Operation,
        boundary: &BoundaryMachineDeclaration,
        plan: &TerminalPlacementLifecyclePlan,
        rejection: InitializationRejection,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        let [extent, original, _profile] = structural_arguments.as_slice() else {
            return Err(malformed());
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        if !self
            .loan_completions_for_operation(operation.id)?
            .is_empty()
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "placement initialization has no mutable borrowed input to restore",
            ));
        }
        if self.result_place_in_use(result.place) || !result.qualification_establishments.is_empty()
        {
            return Err(malformed());
        }
        let program = std::sync::Arc::clone(&self.program);
        let PlacementResultContract::Initialize {
            result: declaration,
            rejected,
            returned,
            failures,
            ..
        } = placement_result_contract(&program.catalogs, plan)
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?
        else {
            return Err(malformed());
        };
        if result.structural_type != declaration.structural_type
            || result.multiplicity != declaration.multiplicity
            || result.qualifications != declaration.qualifications
            || result.projected_qualifications != declaration.projected_qualifications
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "placement rejection result differs from its boundary contract",
            ));
        }
        let [returned_field, reason_field] = rejected.fields.as_slice() else {
            return Err(malformed());
        };
        let mut identity_cursor = self.local_structural_identities.cursor();
        let mut value = TerminalStructuralValue::minted(
            self.local_structural_identities
                .allocate_staged(&mut identity_cursor)?,
            result.structural_type,
            Vec::new(),
        );
        let root = StructuralRuntimePlace::from(&value);
        let mut storage = ConstructorStorage::default();
        storage.stage_case(root.clone(), result.structural_type, rejected.id);
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        let mut transported = BTreeMap::new();
        for (position, (source, field)) in [extent, original].into_iter().zip(returned).enumerate()
        {
            let parameter = boundary
                .structural_parameters
                .get(position)
                .ok_or_else(malformed)?;
            if source.access != StructuralAccess::Owned
                || parameter.access != StructuralAccess::Owned
            {
                return Err(malformed());
            }
            let child = ConstructorValue::resolve(self, source)?;
            if child.structural_type() != parameter.structural_type {
                return Err(malformed());
            }
            if let ConstructorValue::Placed(placed) = &child
                && crate::custody::has_live_loan(&self.reference_referents, placed)
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "placement rejection cannot transfer a borrowed input",
                ));
            }
            let destination_path = vec![
                StructuralPathSegment::Case(rejected.id),
                StructuralPathSegment::Field(returned_field.identity),
                StructuralPathSegment::Field(field.identity),
            ];
            // Storage omits Case selectors; ownership and qualifications keep
            // them, exactly as ordinary mixed/case construction does.
            let destination = StructuralRuntimePlace {
                opaque_identity: value.opaque_identity,
                path: destination_path
                    .iter()
                    .filter(|segment| !matches!(segment, StructuralPathSegment::Case(_)))
                    .cloned()
                    .collect(),
            };
            let transfers = self.stage_constructor_operand(source, &mut frontier, &mut consumed)?;
            if parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                && !transfers
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            storage.stage(self, source, &child, &destination, transfers)?;
            child.append_memberships(&destination_path, &mut value.projected_qualifications);

            let source_claims =
                crate::custody::projected_ownership_claims(&self.live_claims, source)?;
            for (identity, claim) in &source_claims {
                let mut path = destination_path.clone();
                path.extend_from_slice(&claim.path);
                if transported.insert(path, *identity).is_some() {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
            }
            // A partially moved affine input must not be rebuilt as a whole
            // merely because its declared carrier has the expected fields.
            let (whole, projected) = child.memberships();
            let expected = StructuralOperationResult {
                place: source.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                qualifications: whole.to_vec(),
                projected_qualifications: projected.to_vec(),
                qualification_establishments: Vec::new(),
                claims: source_claims
                    .iter()
                    .map(|(claim, held)| StructuralResultClaimBinding {
                        claim: *claim,
                        path: held.path.clone(),
                    })
                    .collect(),
            };
            let selected = |region: &StructuralAffineDiscard| {
                let mut path = destination_path.clone();
                path.extend_from_slice(&region.path);
                storage.case_at_projection(self, &value, &path)
            };
            let mut expected_frontier = BTreeSet::new();
            self.stage_result_affine_frontier(&expected, &mut expected_frontier, &selected)?;
            self.validate_transferred_affine_complement(
                source,
                &expected,
                &expected_frontier,
                &selected,
            )?;
        }
        super::initialization::stage_rejection_failure(
            &mut storage,
            rejection,
            failures,
            reason_field,
            value.opaque_identity,
        )?;
        value.projected_qualifications.sort();
        value.projected_qualifications.dedup();
        storage.validate_projected_memberships(self, &value, &result.projected_qualifications)?;
        let selected = |region: &StructuralAffineDiscard| {
            storage.case_at_projection(self, &value, &region.path)
        };
        let mut claims = crate::custody::complete_claims(
            &self.live_claims,
            structural_arguments,
            completion_receipts,
            &boundary.structural_parameters,
        )?;
        let mut received = BTreeSet::new();
        for binding in &result.claims {
            if !self.constructor_claim_is_active(result, &binding.path, &selected)? {
                continue;
            }
            let original_identity = transported
                .remove(&binding.path)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if !received.insert(original_identity) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            crate::custody::bind_result_claim(
                &mut claims,
                binding,
                result.place,
                result.multiplicity,
            )?;
        }
        if !transported.is_empty() || received.len() != completion_receipts.len() {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
        self.stage_result_affine_frontier(result, &mut frontier, &selected)?;
        // No backing writes occur on rejection. Constructor staging has
        // retained the original descriptor association, bytes and memberships.
        self.commit_constructor_operands(frontier, &consumed, claims)?;
        storage.publish(self)?;
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        self.structural_values.insert(result.place, value)?;
        Ok(())
    }
}

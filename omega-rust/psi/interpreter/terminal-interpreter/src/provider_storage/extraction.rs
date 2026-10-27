//! Whole extraction transfers the resident's original custody and current bytes.
//!
//! A Resident annotation is insufficient: the live owned descriptor must carry
//! the original generational allocation/custody association and exact P/T plan.
//! Decode stages ordinary constructor cells; original claims, qualifications
//! and non-byte bindings travel separately. Every refusing operation precedes
//! retirement. The original allocation and its unused tail are never replaced.

use super::ProviderStorageBinding;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::record::storage::{ConstructorStorage, ConstructorValue};
use crate::values::StructuralRuntimePlace;
use std::collections::BTreeSet;
use terminal_psi::{
    BoundaryMachineDeclaration, Operation, OperationKind, StructuralAffineDiscard,
    StructuralPathQualification, StructuralPathSegment, TerminalPlacementLifecyclePlan,
};
use terminal_semantics::{PlacementResultContract, placement_result_contract};

impl TerminalExecution<'_> {
    pub(crate) fn extract_placement_resident(
        &mut self,
        operation: &Operation,
        boundary: &BoundaryMachineDeclaration,
        plan: &TerminalPlacementLifecyclePlan,
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
        let [source, profile_argument] = structural_arguments.as_slice() else {
            return Err(malformed());
        };
        let [parameter, _] = boundary.structural_parameters.as_slice() else {
            return Err(malformed());
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        if self.result_place_in_use(result.place)
            || !result.qualification_establishments.is_empty()
            || !self
                .loan_completions_for_operation(operation.id)?
                .is_empty()
        {
            return Err(malformed());
        }
        let program = std::sync::Arc::clone(&self.program);
        let PlacementResultContract::MoveOut {
            result: declaration,
            moved,
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
                "extraction result differs from its boundary contract",
            ));
        }
        let [extent_field, value_field] = moved.fields.as_slice() else {
            return Err(malformed());
        };
        let mut identity_cursor = self.local_structural_identities.cursor();
        let mut extent = self.stage_owned_value(
            source,
            plan.storage.carrier,
            parameter.multiplicity,
            &mut identity_cursor,
        )?;
        if extent.claim_paths.as_slice() != [Vec::<StructuralPathSegment>::new()]
            || !extent.value.qualifications.contains(&plan.storage.granted)
            || !extent.value.qualifications.contains(&resident_domain)
            || extent.value.qualifications.contains(&vacant)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let child = ConstructorValue::resolve(self, source)?;
        let ConstructorValue::Placed(original_extent) = &child else {
            return Err(malformed());
        };
        let observed = self.resolve_live_projection(
            profile_argument.place,
            &profile_argument.path,
            profile_argument.access,
        )?;
        let (observed_binding, observed_profile) =
            self.lifecycle_profile(plan, original_extent, &observed)?;
        let home = StructuralRuntimePlace::from(original_extent);
        let binding = self
            .provider_storage
            .descriptors
            .get(&home)
            .copied()
            .ok_or(TerminalInterpretError::ContentTransformationMismatch(
                "extraction has no actual provider allocation",
            ))?;
        if binding.resident == arena::Handle::default()
            || binding != observed_binding
            || !self.is_live_provider_descriptor(&home, &binding)
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "extraction has no live resident custody",
            ));
        }
        let (initialized, custody, compatibility) = self.live_resident_custody(binding.resident)?;
        if initialized.schema != plan.schema
            || custody.value.structural_type != plan.schema
            || initialized.schema_identity != plan.schema_identity
            || initialized.policy_identity != plan.policy_identity
            || initialized.policy_plan_machine_identity != plan.policy_plan_machine_identity
            || initialized.storage != plan.storage
            || initialized.placement != plan.placement
            || initialized.image_fields != plan.image_fields
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "extraction changed the original resident policy or schema",
            ));
        }
        let (allocation, profile, base) =
            self.provider_storage.live_provider_allocation(&binding)?;
        if !self.provider_storage_geometry_matches(&home, &plan.storage, base, binding.length)? {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "extraction changed the original allocation geometry",
            ));
        }
        let required = compatibility.byte_length();
        if plan.placement.layout().size != Some(required) || required > binding.length {
            return Err(malformed());
        }
        let loan = allocation
            .extent
            .loan(binding.offset, binding.length)
            .map_err(|diagnostic| TerminalInterpretError::ResourceProfile {
                boundary: plan.boundary,
                diagnostic: access_plans::AccessPlanDiagnostic(format!(
                    "extraction extent loan: {diagnostic}"
                )),
            })?;
        let supplied = profile.restrict_to_loan(&loan).map_err(|diagnostic| {
            TerminalInterpretError::ResourceProfile {
                boundary: plan.boundary,
                diagnostic,
            }
        })?;
        // Re-observation cannot replace the allocation's original admitted
        // supply, geometry, era or provenance with a new descriptive value.
        if supplied.identity() != observed_profile.identity() {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "extraction profile differs from the original admitted allocation supply",
            ));
        }
        let mut covered = 0;
        for region in supplied.regions() {
            if covered == required {
                break;
            }
            if region.offset != covered
                || !matches!(
                    region.stable,
                    access_plans::StableCapability::Read
                        | access_plans::StableCapability::ReadWrite
                )
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "whole resident extraction requires complete Stable read supply",
                ));
            }
            covered = (region.offset + region.length).min(required);
        }
        if covered != required {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "resident extraction image has incomplete read supply",
            ));
        }
        let start = usize::try_from(binding.offset).map_err(|_| malformed())?;
        let end = binding
            .offset
            .checked_add(required)
            .and_then(|end| usize::try_from(end).ok())
            .ok_or_else(malformed)?;
        let image = allocation.bytes.get(start..end).ok_or_else(malformed)?;
        let mut value = self.mint_staged(&mut identity_cursor, result.structural_type)?;
        let extent_path = vec![
            StructuralPathSegment::Case(moved.id),
            StructuralPathSegment::Field(extent_field.identity),
        ];
        let value_path = vec![
            StructuralPathSegment::Case(moved.id),
            StructuralPathSegment::Field(value_field.identity),
        ];
        let extent_home = StructuralRuntimePlace {
            opaque_identity: value.opaque_identity,
            path: vec![StructuralPathSegment::Field(extent_field.identity)],
        };
        let value_home = StructuralRuntimePlace {
            opaque_identity: value.opaque_identity,
            path: vec![StructuralPathSegment::Field(value_field.identity)],
        };
        let decoded = super::value_image::restore(self, plan, image, &value_home)?;
        let mut storage = ConstructorStorage::new(self.account.as_ref());
        storage.stage_case(
            StructuralRuntimePlace::from(&value),
            result.structural_type,
            moved.id,
        )?;
        storage.stage(self, source, &child, &extent_home, true)?;
        storage.take_provider_descriptor(&extent_home, binding)?;
        storage.stage_resident_image(self, custody, &value_home, decoded)?;
        extent
            .value
            .qualifications
            .retain(|domain| *domain != resident_domain);
        extent.value.qualifications.push(vacant);
        extent.value.qualifications.sort();
        StructuralPathQualification::append_under(
            &extent.value.qualifications,
            &extent.value.projected_qualifications,
            &extent_path,
            &mut value.projected_qualifications,
        );
        StructuralPathQualification::append_under(
            &custody.value.qualifications,
            &custody.value.projected_qualifications,
            &value_path,
            &mut value.projected_qualifications,
        );
        storage.validate_projected_memberships(self, &value, &result.projected_qualifications)?;
        let selected = |region: &StructuralAffineDiscard| {
            storage.case_at_projection(self, &value, &region.path)
        };
        let mut expected_claims = BTreeSet::from([extent_path.clone()]);
        for path in &custody.claim_paths {
            let mut destination = value_path.clone();
            destination.extend_from_slice(path);
            if !expected_claims.insert(destination) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
        let mut claims = crate::custody::complete_claims(
            &self.live_claims,
            structural_arguments,
            completion_receipts,
            &boundary.structural_parameters,
        )?;
        for binding in &result.claims {
            if !self.constructor_claim_is_active(result, &binding.path, &selected)? {
                continue;
            }
            if !expected_claims.remove(&binding.path) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            crate::custody::bind_result_claim(
                &mut claims,
                binding,
                result.place,
                result.multiplicity,
            )?;
        }
        if !expected_claims.is_empty() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        if !self.stage_constructor_operand(source, &mut frontier, &mut consumed)? {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        self.stage_result_affine_frontier(result, &mut frontier, &selected)?;
        let actual_affine = frontier
            .iter()
            .filter(|region| region.place == result.place)
            .filter_map(|region| {
                region
                    .path
                    .strip_prefix(value_path.as_slice())
                    .map(|path| (path.to_vec(), region.structural_type))
            })
            .collect::<BTreeSet<_>>();
        let expected_affine = custody
            .affine_regions
            .iter()
            .map(|region| (region.path.clone(), region.structural_type))
            .collect::<BTreeSet<_>>();
        if actual_affine != expected_affine {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
        let storage = storage.into_detached()?;
        storage.install(self)?;
        self.structural_values.insert(result.place, value)?;
        self.provider_storage.descriptors.insert_vacant(
            extent_home,
            ProviderStorageBinding {
                resident: arena::Handle::default(),
                ..binding
            },
        )?;
        // All source and receiving obligations have joined. Decoded data and
        // retained non-byte bindings now replace the private snapshot together.
        let _original = std::mem::take(self.provider_storage.residents.get_mut(binding.resident));
        self.provider_storage.residents.free(binding.resident);
        retirement.commit(self)?;
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        Ok(())
    }
}

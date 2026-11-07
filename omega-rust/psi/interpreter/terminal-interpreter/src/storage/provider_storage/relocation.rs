//! Relocate the actual retained values without extracting or reconstructing them.
//! Resource checks and result custody are staged before the destination write.
//! The resident handle survives the move; only its backing window changes.

use super::initialization::InitializationRejection;
use super::{ProviderStorageBinding, ProviderStorageSlot, ResidentStorageSlot};
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::{
    StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
};
use crate::engine::custody::bind_result_claim;
use crate::engine::execution::{TerminalExecution, operation_kind};
use crate::storage::byte_sequences::storage::ByteSequenceStorage;
use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
use crate::storage::record::storage::{ConstructorStorage, ConstructorValue};
use semantic_vocabulary::IntegerValue;
use std::collections::BTreeSet;
use terminal_psi::{
    BoundaryMachineDeclaration, Operation, OperationKind, StructuralPathQualification,
    StructuralPathSegment, TerminalAccessLifecyclePlan, TerminalAccessOwner,
};
use terminal_semantics::{AccessResultContract, access_result_contract};

impl TerminalExecution<'_> {
    pub(crate) fn relocate_access_run(
        &mut self,
        operation: &Operation,
        boundary: &BoundaryMachineDeclaration,
        plan: &TerminalAccessLifecyclePlan,
    ) -> Result<(), TerminalInterpretError> {
        operation_kind!(operation => BoundaryCall {
            structural_arguments, completion_receipts, ..
        });
        let [source, destination, source_profile, destination_profile] =
            structural_arguments.as_slice()
        else {
            return Err(malformed());
        };
        let TerminalAccessOwner::Run {
            count: count_field, ..
        } = plan.storage.owner
        else {
            return Err(malformed());
        };
        let (vacant, live) = plan
            .storage
            .owned_content()
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
        let program = std::sync::Arc::clone(&self.program);
        let owner = self.resolve_live_projection(source.place, &source.path, source.access)?;
        if !owner.qualifications.contains(&live) {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let count_value = self
            .structural_scalar_fields
            .get(&StructuralScalarRuntimeField::member(
                StructuralRuntimePlace::from(&owner),
                count_field,
            ))
            .ok_or_else(malformed)?
            .read()?;
        let TerminalScalarValue::Integer {
            value: IntegerValue::Unsigned(count),
            ..
        } = count_value
        else {
            return Err(malformed());
        };
        let count = u64::try_from(count).map_err(|_| malformed())?;
        let backing_path = plan
            .storage
            .owner_storage_path(&program.catalogs.structural_types)
            .ok_or_else(malformed)?;
        let mut backing_source = source.clone();
        backing_source.path.extend_from_slice(&backing_path);
        let source_child = ConstructorValue::resolve(self, &backing_source)?;
        let destination_child = ConstructorValue::resolve(self, destination)?;
        let (ConstructorValue::Access(source_extent), ConstructorValue::Access(destination_extent)) =
            (&source_child, &destination_child)
        else {
            return Err(malformed());
        };
        let source_observation = self.resolve_live_projection(
            source_profile.place,
            &source_profile.path,
            source_profile.access,
        )?;
        let destination_observation = self.resolve_live_projection(
            destination_profile.place,
            &destination_profile.path,
            destination_profile.access,
        )?;
        let (source_binding, source_supply) =
            self.lifecycle_profile(plan, 0, source_extent, &source_observation)?;
        let (destination_binding, destination_supply) =
            self.lifecycle_profile(plan, 1, destination_extent, &destination_observation)?;
        if (count == 0) != (source_binding.resident == arena::Handle::default()) {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "run relocation count disagrees with retained cell custody",
            ));
        }
        if count != 0 {
            let (initialized, _, _) = self.live_resident_custody(source_binding.resident)?;
            if initialized.schema != plan.schema
                || initialized.schema_identity != plan.schema_identity
                || initialized.policy_identity != plan.policy_identity
                || initialized.policy_plan_machine_identity != plan.policy_plan_machine_identity
                || !initialized.storage.same_custody(&plan.storage)
                || initialized.placement != plan.placement
                || initialized.image_fields != plan.image_fields
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "relocation changed the retained cell policy or schema",
                ));
            }
        }
        let (source_allocation, source_admission, _) = self
            .provider_storage
            .live_provider_allocation(&source_binding)?;
        let (destination_allocation, destination_admission, destination_base) = self
            .provider_storage
            .live_provider_allocation(&destination_binding)?;
        let source_end = source_binding
            .offset
            .checked_add(source_binding.length)
            .ok_or_else(malformed)?;
        let destination_end = destination_binding
            .offset
            .checked_add(destination_binding.length)
            .ok_or_else(malformed)?;
        if source_binding.allocation == destination_binding.allocation
            && source_binding.offset < destination_end
            && destination_binding.offset < source_end
        {
            return self.reject_access_transfer(
                operation,
                boundary,
                plan,
                InitializationRejection::IncompatibleSupply,
            );
        }
        let source_loan = source_allocation
            .extent
            .loan(source_binding.offset, source_binding.length)
            .map_err(|error| TerminalInterpretError::ResourceProfile {
                boundary: plan.boundary,
                diagnostic: access_plans::AccessPlanDiagnostic(format!(
                    "relocation source loan: {error}"
                )),
            })?;
        // Restriction only identifies the admitted interval. Exclusive write
        // custody is established by the owned destination transaction below.
        let destination_loan = destination_allocation
            .extent
            .loan(destination_binding.offset, destination_binding.length)
            .map_err(|error| TerminalInterpretError::ResourceProfile {
                boundary: plan.boundary,
                diagnostic: access_plans::AccessPlanDiagnostic(format!(
                    "relocation destination loan: {error}"
                )),
            })?;
        let admitted_source =
            source_admission
                .restrict_to_loan(&source_loan)
                .map_err(|diagnostic| TerminalInterpretError::ResourceProfile {
                    boundary: plan.boundary,
                    diagnostic,
                })?;
        let admitted_destination = destination_admission
            .restrict_to_loan(&destination_loan)
            .map_err(|diagnostic| TerminalInterpretError::ResourceProfile {
                boundary: plan.boundary,
                diagnostic,
            })?;
        // The selected provider's description and the actual allocation must
        // both support the move. Neither can amplify the other one's rights.
        let selected = access_plans::validate_run_relocation_resources(
            &plan.placement,
            count,
            destination_base,
            &source_supply,
            &destination_supply,
        );
        let admitted = access_plans::validate_run_relocation_resources(
            &plan.placement,
            count,
            destination_base,
            &admitted_source,
            &admitted_destination,
        );
        let (Ok(_), Ok(compatibility)) = (selected, admitted) else {
            return self.reject_access_transfer(
                operation,
                boundary,
                plan,
                InitializationRejection::IncompatibleSupply,
            );
        };
        let source_start = usize::try_from(source_binding.offset).map_err(|_| malformed())?;
        let source_end = usize::try_from(source_end).map_err(|_| malformed())?;
        let source_bytes = source_allocation
            .bytes
            .read()
            .map_err(TerminalInterpretError::ResidentStorage)?;
        let image = ByteSequenceStorage::copy(
            source_bytes
                .get(source_start..source_end)
                .ok_or_else(malformed)?,
            self.account.as_ref(),
        )?;
        drop(source_bytes);

        let result = operation.result.structural().ok_or_else(malformed)?;
        if self.result_place_in_use(result.place)
            || !result.qualification_establishments.is_empty()
            || !self
                .loan_completions_for_operation(operation.id)?
                .is_empty()
        {
            return Err(malformed());
        }
        let AccessResultContract::Relocate {
            result: declaration,
            ready,
            moved,
            ..
        } = access_result_contract(&program.catalogs, plan)
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
                "relocation result differs from its contract",
            ));
        }
        let [ready_field] = ready.fields.as_slice() else {
            return Err(malformed());
        };
        let mut identity_cursor = self.local_structural_identities.cursor();
        for (argument, parameter) in [source, destination]
            .into_iter()
            .zip(&boundary.structural_parameters)
        {
            let retained = self.stage_owned_value(
                argument,
                parameter.structural_type,
                parameter.multiplicity,
                &mut identity_cursor,
            )?;
            if retained.claim_paths.as_slice() != [Vec::<StructuralPathSegment>::new()] {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
        let mut value = self.mint_staged(&mut identity_cursor, result.structural_type)?;
        let result_path = vec![
            StructuralPathSegment::Case(ready.id),
            StructuralPathSegment::Field(ready_field.identity),
        ];
        let mut run_path = result_path.clone();
        run_path.push(StructuralPathSegment::Field(moved[0].identity));
        let mut vacated_path = result_path;
        vacated_path.push(StructuralPathSegment::Field(moved[1].identity));
        let run_home = StructuralRuntimePlace {
            opaque_identity: value.opaque_identity,
            path: run_path.clone(),
        };
        let mut new_backing_home = run_home.clone();
        new_backing_home.path.extend_from_slice(&backing_path);
        let vacated_home = StructuralRuntimePlace {
            opaque_identity: value.opaque_identity,
            path: vacated_path.clone(),
        };
        let mut storage = ConstructorStorage::new(self.account.as_ref());
        storage.stage_case(
            StructuralRuntimePlace::from(&value),
            result.structural_type,
            ready.id,
        )?;
        storage.stage(self, &backing_source, &source_child, &vacated_home, true)?;
        storage.stage(
            self,
            destination,
            &destination_child,
            &new_backing_home,
            true,
        )?;
        storage.take_provider_descriptor(&vacated_home, source_binding)?;
        storage.take_provider_descriptor(&new_backing_home, destination_binding)?;
        storage.stage_scalar_endpoint(
            &PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField::member(
                run_home,
                count_field,
            )),
            count_value,
        )?;
        StructuralPathQualification::append_under(
            &[live],
            &[],
            &run_path,
            &mut value.projected_qualifications,
        );
        StructuralPathQualification::append_under(
            &[plan.storage.granted],
            &[],
            &new_backing_home.path,
            &mut value.projected_qualifications,
        );
        StructuralPathQualification::append_under(
            &[plan.storage.granted, vacant],
            &[],
            &vacated_path,
            &mut value.projected_qualifications,
        );
        value.projected_qualifications.sort();
        storage.validate_projected_memberships(self, &value, &result.projected_qualifications)?;
        let selected = |region: &terminal_psi::StructuralAffineDiscard| {
            storage.case_at_projection(self, &value, &region.path)
        };
        let mut claims = crate::engine::custody::complete_claims(
            &self.live_claims,
            structural_arguments,
            completion_receipts,
            &boundary.structural_parameters,
        )?;
        let mut expected = BTreeSet::from([run_path, vacated_path]);
        for binding in &result.claims {
            if !self.constructor_claim_is_active(result, &binding.path, &selected)? {
                continue;
            }
            if !expected.remove(&binding.path) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
        }
        if !expected.is_empty() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        for argument in [source, destination] {
            if !self.stage_constructor_operand(argument, &mut frontier, &mut consumed)? {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
        self.stage_result_affine_frontier(result, &mut frontier, &selected)?;
        let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
        storage.into_detached()?.install(self)?;
        self.structural_values.insert(result.place, value)?;
        self.provider_storage.descriptors.insert_vacant(
            vacated_home,
            ProviderStorageBinding {
                resident: arena::Handle::default(),
                ..source_binding
            },
        )?;
        self.provider_storage.descriptors.insert_vacant(
            new_backing_home,
            ProviderStorageBinding {
                resident: source_binding.resident,
                ..destination_binding
            },
        )?;
        let ProviderStorageSlot::Live { allocation, .. } = self
            .provider_storage
            .allocations
            .get_mut(destination_binding.allocation)
        else {
            return Err(malformed());
        };
        let destination_start =
            usize::try_from(destination_binding.offset).map_err(|_| malformed())?;
        let destination_end = usize::try_from(destination_end).map_err(|_| malformed())?;
        let mut bytes = allocation
            .bytes
            .write()
            .map_err(TerminalInterpretError::ResidentStorage)?;
        let destination_bytes = bytes
            .get_mut(destination_start..destination_end)
            .ok_or_else(malformed)?;
        let resident = match compatibility {
            Some(compatibility) => {
                let ResidentStorageSlot::Live {
                    window,
                    compatibility: current,
                    ..
                } = self
                    .provider_storage
                    .residents
                    .get_mut(source_binding.resident)
                else {
                    return Err(malformed());
                };
                Some((window, current, compatibility))
            }
            None => None,
        };
        // No destructor, decode, or new cell claim participates in the commit.
        destination_bytes.copy_from_slice(image.bytes());
        if let Some((window, current, compatibility)) = resident {
            *window = destination_binding;
            *current = compatibility;
        }
        drop(bytes);
        retirement.commit(self)?;
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        Ok(())
    }
}

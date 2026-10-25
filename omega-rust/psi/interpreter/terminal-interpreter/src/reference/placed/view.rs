//! Fallible borrowed establishment publishes a descriptor only in Ready.
//! The descriptor retains the original owner and independently admitted supply;
//! the temporary profile's storage never becomes its referent. No byte read,
//! initialization, claim transfer or content qualification occurs here.

use super::{PlacedReference, PlacedStorage};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::record::storage::ConstructorStorage;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralAffineDiscard, StructuralPathSegment,
    TerminalPlacementLifecyclePlan,
};

impl TerminalExecution<'_> {
    pub(crate) fn establish_placement_view(
        &mut self,
        operation: &Operation,
        plan: &TerminalPlacementLifecyclePlan,
    ) -> Result<(), TerminalInterpretError> {
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
        let result = operation.result.structural().ok_or_else(malformed)?;
        if !matches!(source.access, terminal_psi::reading_borrow_pair_pattern!())
            || profile_argument.access != StructuralAccess::SharedBorrow
            || !completion_receipts.is_empty()
            || !result.claims.is_empty()
            || !result.qualification_establishments.is_empty()
            || self.result_place_in_use(result.place)
            || !self
                .loan_completions_for_operation(operation.id)?
                .is_empty()
        {
            return Err(malformed());
        }
        let program = std::sync::Arc::clone(&self.program);
        terminal_semantics::validate_placement_view_content(&program.catalogs, plan)
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
        let terminal_semantics::PlacementResultContract::View {
            result: declared,
            ready,
            rejected,
            failures,
            carrier,
            access,
        } = terminal_semantics::placement_result_contract(&program.catalogs, plan)
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?
        else {
            return Err(malformed());
        };
        if source.access != access
            || result.structural_type != declared.structural_type
            || result.multiplicity != declared.multiplicity
            || result.qualifications != declared.qualifications
            || result.projected_qualifications != declared.projected_qualifications
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "borrowed placement changed its declared result contract",
            ));
        }
        let backing = self
            .resolve_reference_call_arguments(std::slice::from_ref(source))?
            .pop()
            .ok_or_else(malformed)?;
        let profile = self.resolve_live_projection(
            profile_argument.place,
            &profile_argument.path,
            profile_argument.access,
        )?;
        let (binding, supplied) = self.lifecycle_profile(plan, &backing, &profile)?;
        let assessment = self
            .provider_storage
            .prepare_view(binding, &supplied, &plan.placement)?;
        let mut identity_cursor = self.local_structural_identities.cursor();
        let value = TerminalStructuralValue::minted(
            self.local_structural_identities
                .allocate_staged(&mut identity_cursor)?,
            result.structural_type,
            Vec::new(),
        );
        let root = StructuralRuntimePlace::from(&value);
        let mut storage = ConstructorStorage::default();
        match assessment {
            Ok(compatibility) => {
                let [field] = ready.fields.as_slice() else {
                    return Err(malformed());
                };
                storage.stage_case(root.clone(), result.structural_type, ready.id);
                let mut presentation = backing.clone();
                presentation.structural_type = carrier;
                presentation.qualifications.clear();
                presentation.projected_qualifications.clear();
                let captured = PlacedReference {
                    storage: PlacedStorage::External {
                        carrier,
                        owner: StructuralRuntimePlace::from(&backing),
                        binding,
                        compatibility,
                    },
                    presentation,
                    access,
                    selected_field: None,
                    projection: Vec::new(),
                    source: backing.clone(),
                };
                self.validate_placed_backing(&captured)?;
                storage.stage_reference(
                    StructuralRuntimePlace {
                        opaque_identity: value.opaque_identity,
                        path: vec![StructuralPathSegment::Field(field.identity)],
                    },
                    crate::reference::ReferenceReferent {
                        view: backing,
                        primitive_loan: None,
                        collection_loan: None,
                        byte_region: None,
                        byte_owner: None,
                        aggregate_view: None,
                        placed: Some(captured),
                    },
                )?;
            }
            Err(rejection) => {
                let [_, reason_field] = rejected.fields.as_slice() else {
                    return Err(malformed());
                };
                storage.stage_case(root.clone(), result.structural_type, rejected.id);
                crate::provider_storage::initialization::stage_rejection_failure(
                    &mut storage,
                    rejection,
                    failures,
                    reason_field,
                    value.opaque_identity,
                )?;
            }
        }
        storage.validate_projected_memberships(self, &value, &result.projected_qualifications)?;
        let mut frontier = self.live_affine_frontier.clone();
        self.stage_result_affine_frontier(
            result,
            &mut frontier,
            &|region: &StructuralAffineDiscard| {
                storage.case_at_projection(self, &value, &region.path)
            },
        )?;
        let storage = storage.into_detached()?;
        // All refusing work is complete. Rejection publishes no reference;
        // Ready's one descriptor keeps the source loan visible to retirement.
        storage.publish(self);
        self.live_affine_frontier = frontier;
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        self.structural_values.insert(result.place, value)?;
        Ok(())
    }
}

//! Retire zero occupied cells while preserving the exact backing descriptor.
//!
//! This operation never consults a provider and never writes backing bytes.
//! Stage the existing descriptor and the complete custody transaction before
//! consuming the run, just as other owned structural moves do.

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::{
    StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
};
use crate::engine::custody::bind_result_claim;
use crate::engine::execution::{TerminalExecution, operation_kind};
use crate::storage::record::storage::{ConstructorStorage, ConstructorValue};
use semantic_vocabulary::{IntegerValue, StructuralFieldId};
use terminal_psi::{BoundaryMachineDeclaration, Operation, OperationKind, StructuralPathSegment};

impl TerminalExecution<'_> {
    pub(crate) fn release_empty_run(
        &mut self,
        operation: &Operation,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<(), TerminalInterpretError> {
        operation_kind!(operation => BoundaryCall {
            structural_arguments, completion_receipts, ..
        });
        let [source] = structural_arguments.as_slice() else {
            return Err(malformed());
        };
        let program = std::sync::Arc::clone(&self.program);
        let contract = terminal_semantics::empty_run_release_contract(
            &program.catalogs.structural_types,
            boundary,
        )
        .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
        let result = operation.result.structural().ok_or_else(malformed)?;
        if self.result_place_in_use(result.place)
            || !result.qualification_establishments.is_empty()
            || !self
                .loan_completions_for_operation(operation.id)?
                .is_empty()
            || result.structural_type != contract.result.structural_type
            || result.multiplicity != contract.result.multiplicity
            || result.qualifications != contract.result.qualifications
            || result.projected_qualifications != contract.result.projected_qualifications
        {
            return Err(malformed());
        }
        let owner = self.resolve_live_projection(source.place, &source.path, source.access)?;
        if crate::engine::custody::has_live_loan(self.reference_referents.as_map(), &owner) {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let mut backing_source = source.clone();
        backing_source
            .path
            .push(StructuralPathSegment::Field(contract.storage));
        let child = ConstructorValue::resolve(self, &backing_source)?;
        let ConstructorValue::Access(backing) = &child else {
            return Err(malformed());
        };
        for requirement in &boundary.requires {
            let mut path = source.path.clone();
            path.extend_from_slice(&requirement.path);
            let subject = self.resolve_live_projection(source.place, &path, source.access)?;
            if requirement.argument_index != 0
                || !subject.qualifications.contains(&requirement.domain)
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
        if !self.empty_run_zero_field(&StructuralRuntimePlace::from(&owner), contract.count)?
            || !self
                .empty_run_zero_field(&StructuralRuntimePlace::from(backing), contract.length)?
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "empty-run release requires zero cells and zero occupied bytes",
            ));
        }
        // A zero-footprint element still has custody. Do not infer emptiness
        // from the descriptor's byte count or discard a retained resident.
        if self
            .provider_storage
            .descriptors
            .get(&StructuralRuntimePlace::from(backing))
            .is_some_and(|binding| binding.resident != arena::Handle::default())
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "empty-run release cannot discard retained resident custody",
            ));
        }
        let mut identity_cursor = self.local_structural_identities.cursor();
        let parameter = &boundary.structural_parameters[0];
        let retained = self.stage_owned_value(
            source,
            parameter.structural_type,
            parameter.multiplicity,
            &mut identity_cursor,
        )?;
        if retained.claim_paths.as_slice() != [Vec::<StructuralPathSegment>::new()]
            || completion_receipts.len() != 1
            || backing
                .qualifications
                .iter()
                .any(|domain| !result.qualifications.contains(domain))
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let mut claims = crate::engine::custody::complete_claims(
            &self.live_claims,
            structural_arguments,
            completion_receipts,
            &boundary.structural_parameters,
        )?;
        let [binding] = result.claims.as_slice() else {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        };
        if !binding.path.is_empty() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
        let mut value = self.mint_staged(&mut identity_cursor, result.structural_type)?;
        value.qualifications = result.qualifications.clone();
        value.projected_qualifications = backing.projected_qualifications.clone();
        let mut storage = ConstructorStorage::new(self.account.as_ref());
        storage.stage(
            self,
            &backing_source,
            &child,
            &StructuralRuntimePlace::from(&value),
            true,
        )?;
        storage.validate_projected_memberships(self, &value, &result.projected_qualifications)?;
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        if !self.stage_constructor_operand(source, &mut frontier, &mut consumed)? {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        self.stage_result_affine_frontier(result, &mut frontier, &|region| {
            storage.case_at_projection(self, &value, &region.path)
        })?;
        let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
        storage.into_detached()?.install(self)?;
        self.structural_values.insert(result.place, value)?;
        retirement.commit(self)?;
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        Ok(())
    }

    fn empty_run_zero_field(
        &self,
        home: &StructuralRuntimePlace,
        field: StructuralFieldId,
    ) -> Result<bool, TerminalInterpretError> {
        let value = self
            .structural_scalar_fields
            .get(&StructuralScalarRuntimeField::member(home.clone(), field))
            .map(crate::storage::scalar_storage::ScalarCell::read)
            .transpose()?;
        Ok(matches!(
            value,
            Some(TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(0),
                ..
            })
        ))
    }
}

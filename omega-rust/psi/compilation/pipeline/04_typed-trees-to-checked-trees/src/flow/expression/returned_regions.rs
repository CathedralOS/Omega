//! Local dispatch results retain the slot's admitted enclosing regions.
//! Every table realization must refine that public WithinRegion contract;
//! lifetime tags alone, particularly provider-owned views, promise no such
//! geometry. Keep these regions separate from exact descriptor suppliers:
//! exclusion and write invalidation may use them, but scalar identity, contents
//! and permission ancestry may not. One unresolved region withholds coverage.

use super::Execution;
use arena::Handle;
use checked_trees::{
    CheckedReferenceOriginProjection, CheckedReferenceOriginRoot, CheckedUnitStructuralPathSegment,
    FlowOperandFact, FlowOperandRetainedRegion, FlowOperandRetention,
};
use facts::PlaceSegment;
use semantic_vocabulary::ReferenceOriginExtent;
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use typed_trees::types::TypeReferenceHandle;

impl Execution<'_, '_, '_> {
    pub(super) fn capture_returned_region_descriptor(
        &mut self,
        call: &TableCallExpression,
        inputs: &[(ExpressionHandle, FlowOperandFact)],
        returned: TypeReferenceHandle,
        path: &[PlaceSegment],
        retention: Handle<FlowOperandRetention>,
        source_snapshot: Handle<checked_trees::FlowStorageSnapshot>,
    ) -> Option<Handle<FlowOperandFact>> {
        // Local dyn has an independently enforced result-region interface.
        // Opaque/boundary calls do not acquire it from lifetime correspondence.
        let target = validation::call_target_symbol(self.program, call);
        let slot = self.program.dynamic_slot_by_symbol(target)?;
        let origins = validation::reference_result_custody::declared_return_sources(
            self.program,
            &slot.parameters,
            slot.return_type,
        )
        .ok()?;
        let reference =
            validation::project_type_reference_from_segments(self.program, returned, path)?;
        validation::reference_shell(self.program, reference)?;
        let mut regions = Vec::new();
        for origin in origins {
            let output_path = validation::reference_result_custody::loan_place_segments(
                self.program,
                returned,
                &origin.path,
            )?;
            if output_path != path {
                continue;
            }
            if origin.source.extent != ReferenceOriginExtent::WithinRegion {
                return None;
            }
            let CheckedReferenceOriginRoot::Parameter { index } = origin.source.root else {
                return None;
            };
            let (position, parameter) = slot
                .parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    !parameter.is_const
                        && self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                })
                .nth(usize::try_from(index).ok()?)?;
            let (_, actual) = inputs.get(position)?;
            // Canonical place coordinates look through reference shells. The
            // selected descriptor's typed identity below must still match the
            // public region endpoint; dropping a crossing cannot turn an outer
            // reference into its stored inner descriptor.
            let projection = validation::reference_result_custody::origin_place_segments(
                self.program,
                actual.type_reference,
                origin
                    .source
                    .path
                    .iter()
                    .filter(|projection| {
                        !matches!(
                            projection,
                            CheckedReferenceOriginProjection::Exact(
                                CheckedUnitStructuralPathSegment::Referent
                            )
                        )
                    })
                    .cloned(),
            )?;
            if !validation::storage_origin_types_match(
                self.program,
                actual.type_reference,
                parameter.type_reference,
            ) {
                return None;
            }
            let expected = region_descriptor_type(
                self.program,
                parameter.type_reference,
                &origin.source.path,
            )?;
            for (capture, projection) in
                self.call_slot_values(actual, &projection, source_snapshot)?
            {
                let selected = validation::project_type_reference_from_segments(
                    self.program,
                    capture.type_reference,
                    &projection,
                )?;
                validation::reference_shell(self.program, selected)?;
                if !validation::storage_origin_types_match(self.program, selected, expected) {
                    return None;
                }
                let operand = self.context.control.operands.insert(capture);
                let projection = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(projection);
                regions.push(FlowOperandRetainedRegion {
                    operand,
                    projection,
                });
            }
        }
        if regions.is_empty() || !self.context.control.operand_retentions.is_valid(retention) {
            return None;
        }
        let profile = *self.context.control.operand_retentions.get(retention);
        let enclosing_regions = self
            .context
            .control
            .operand_retained_regions
            .insert_many(regions);
        let retention = self
            .context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                enclosing_regions,
                ..profile
            });
        Some(self.context.control.operands.insert(FlowOperandFact {
            type_reference: reference,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
            storage_snapshot: self.storage_snapshot,
            retention,
            ..Default::default()
        }))
    }
}

/// A regional source may end at a descriptor or its referent. Preserve every
/// explicit reference crossing while finding that descriptor's declared type:
/// `&&[u8]` enclosing a stored `&[u8]` is not the latter's backing region.
fn region_descriptor_type(
    program: &dyn validation::CheckingInput,
    mut reference: TypeReferenceHandle,
    path: &[CheckedReferenceOriginProjection],
) -> Option<TypeReferenceHandle> {
    let project = |reference, pending: &[CheckedReferenceOriginProjection]| {
        let segments = validation::reference_result_custody::origin_place_segments(
            program,
            reference,
            pending.iter().cloned(),
        )?;
        validation::project_type_reference_from_segments(program, reference, &segments)
    };
    let mut pending = Vec::new();
    let mut descriptor = TypeReferenceHandle::invalid();
    for projection in path {
        if matches!(
            projection,
            CheckedReferenceOriginProjection::Exact(CheckedUnitStructuralPathSegment::Referent)
        ) {
            reference = project(reference, &pending)?;
            pending.clear();
            descriptor = reference;
            reference = validation::reference_shell(program, reference)?.referee;
        } else {
            pending.push(projection.clone());
        }
    }
    let ended_at_referent = pending.is_empty() && descriptor.is_valid();
    reference = project(reference, &pending)?;
    if validation::reference_shell(program, reference).is_some() {
        Some(reference)
    } else {
        ended_at_referent.then_some(descriptor)
    }
}

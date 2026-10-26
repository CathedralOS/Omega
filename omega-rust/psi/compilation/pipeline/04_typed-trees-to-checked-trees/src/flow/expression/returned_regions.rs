//! Completed results retain body-derived or public local-dispatch regions.
//! Every table realization must refine its public WithinRegion contract;
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
        let (parameters, origins) =
            region_sources(self.program, call, self.context.call_frames, returned, path)?;
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
            let (position, parameter) = parameters
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

/// Choose geometric authority before binding it to completed actuals. A public
/// local slot supplies its admitted interface; an ordinary call supplies its
/// complete body relation. An opaque/provider signature supplies neither.
fn region_sources<'program>(
    program: &'program dyn validation::CheckingInput,
    call: &TableCallExpression,
    frames: Option<&validation::CallFrameResolver<'_>>,
    returned: TypeReferenceHandle,
    requested_path: &[PlaceSegment],
) -> Option<(
    &'program [typed_trees::signature::StateParameter],
    Vec<checked_trees::CheckedReturnedReferencePlan>,
)> {
    let target = validation::call_target_symbol(program, call);
    if let Some(slot) = program.dynamic_slot_by_symbol(target) {
        return Some((
            &slot.parameters,
            validation::reference_result_custody::declared_return_sources(
                program,
                &slot.parameters,
                slot.return_type,
            )
            .ok()?,
        ));
    }
    let state = validation::call_target_state(program, target)?;
    let machine = program.machine_holding_state(state.symbol)?;
    if !machine.body_is_present
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
    {
        return None;
    }
    let parameters = program.state_parameters(state);
    let mut origins = Vec::new();
    for origin in frames?.reference_result_leaf_origins(state.symbol)? {
        let (output, selected, _) = validation::reference_result_custody::reference_origin_path(
            program,
            state.return_type,
            &origin.result_path,
        )?;
        let output = output
            .into_iter()
            .map(|projection| match projection {
                CheckedReferenceOriginProjection::Exact(segment) => Some(segment),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        // Coverage belongs to one returned leaf. Another field's image or
        // missing relation cannot veto it; every matching alternative can.
        if validation::reference_result_custody::loan_place_segments(program, returned, &output)?
            != requested_path
        {
            continue;
        }

        // A presentation or unresolved alternative needs its own evidence.
        // Lifetime correspondence cannot fill in the missing geometry.
        if origin.source.is_image_storage
            || !origin.source.recasts.is_empty()
            || origin.source.representation.is_some()
        {
            return None;
        }
        let (index, parameter) = parameters
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == origin.source.root)?;
        let (projection, endpoint, extent) =
            validation::reference_result_custody::reference_origin_path(
                program,
                parameter.type_reference,
                &origin.source.segments,
            )?;
        let extent = match extent {
            ReferenceOriginExtent::WithinRegion => extent,
            // An exact descriptor also lies in its own enclosing footprint.
            // Weakening it here establishes no pointer or scalar equality.
            ReferenceOriginExtent::Projected
                if validation::reference_shell(program, endpoint).is_some() =>
            {
                ReferenceOriginExtent::WithinRegion
            }
            _ => return None,
        };
        origins.push(checked_trees::CheckedReturnedReferencePlan {
            path: output,
            source: checked_trees::CheckedReferenceOriginSourcePlan {
                root: CheckedReferenceOriginRoot::Parameter {
                    index: u32::try_from(index).ok()?,
                },
                path: projection,
                extent,
                type_identity: program.normalized_type_identity(endpoint),
                access: validation::reference_shell(program, selected)?
                    .access
                    .into(),
            },
        });
    }
    Some((parameters, origins))
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

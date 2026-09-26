//! The checks one internal Unit call's roster must pass once its facts are
//! established: the call site, each structural argument's source, bytes and
//! path, the projected arguments the call leaves unsettled, and its claim
//! transfers.

use super::{
    exact_borrowed_projection, expected_affine_scalar_record_argument_bytes,
    expected_projected_copy_bytes,
};
use abstract_operations_to_target_operations::calling_conventions::ValueShape;
use post_allocation_machine_to_selected_form_encoding::machine_code::SemanticCodeSite;

use super::InternalUnitCallCustody;
use super::call_facts::{CallInputs, CallSpan, ProjectionFacts, StackFacts};

/// Where one structural argument's custody may come from.
#[derive(Clone, Copy)]
struct ArgumentSources {
    /// The parameter home the argument reads, at the argument's stack offset.
    parameter_source: bool,
    /// The call's exact projected affine result.
    result_source: bool,
    /// A trivial affine local established before the call and passed once.
    local_source: bool,
    /// An affine scalar record established before the call and passed owned
    /// once.
    affine_scalar_record_source: bool,
}

impl InternalUnitCallCustody<'_> {
    /// Whether the argument at `index` is an exact borrowed projection of the
    /// parameter home it reads into the callee's Unit parameter.
    pub(super) fn exact_borrowed_argument(
        &self,
        index: usize,
        argument: &post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitCallArgumentRecord,
    ) -> bool {
        let CallInputs {
            parameter_homes,
            callee_unit_parameters,
            affine_cleanup,
            ..
        } = self.inputs;
        parameter_homes
            .iter()
            .find(|home| home.place == argument.place)
            .zip(callee_unit_parameters.get(index))
            .zip(affine_cleanup)
            .is_some_and(|((source, destination), cleanup)| {
                exact_borrowed_projection(argument, source, destination, &cleanup.structural_types)
            })
    }

    /// Whether the call site is malformed: empty bytes, a relocation outside
    /// them, an owner missing from the provenance or not attributed exactly
    /// once to these bytes, a plan with the wrong parameter count, or
    /// arguments out of code order.
    pub(super) fn call_site_is_malformed(&self) -> bool {
        let CallInputs {
            provenance,
            attribution,
            custody,
            ..
        } = self.inputs;
        let CallSpan {
            relocation,
            end,
            relocation_end,
            ..
        } = self.span;
        let StackFacts { operation, .. } = self.stacks;
        let expected_plan = &self.expected_plan;
        let scalar_count = custody.scalar_arguments.len();
        custody.byte_count == 0
            || custody.code_offset > relocation.offset
            || relocation_end > end
            || !provenance.operations.contains(&operation)
            || attribution
                .iter()
                .filter(|attribution| {
                    attribution.site == SemanticCodeSite::Operation(operation)
                        && attribution.operation_ordinal == custody.operation_ordinal
                        && attribution.code_offset == custody.code_offset
                        && attribution.byte_count == custody.byte_count
                })
                .count()
                != 1
            || expected_plan.parameters.len() != scalar_count + custody.arguments.len()
            || custody.arguments.windows(2).any(|pair| {
                pair[0]
                    .code_offset
                    .checked_add(pair[0].byte_count)
                    .is_none_or(|end| end > pair[1].code_offset)
            })
    }

    /// Whether any structural argument's custody is malformed.
    pub(super) fn arguments_are_malformed(&self) -> bool {
        let CallInputs { custody, .. } = self.inputs;
        let expected_plan = &self.expected_plan;
        let scalar_count = custody.scalar_arguments.len();
        custody
            .arguments
            .iter()
            .zip(&expected_plan.parameters[scalar_count..])
            .enumerate()
            .any(|(argument_index, (argument, destination))| {
                self.argument_is_malformed(argument_index, argument, destination)
            })
    }

    /// Whether one structural argument's custody is malformed: it must land
    /// in the plan's placement with the call's stack bytes, come from a known
    /// source, carry exactly the bytes at its code offset inside the call,
    /// stay inside its source placement, and project a well-formed path.
    fn argument_is_malformed(
        &self,
        argument_index: usize,
        argument: &post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitCallArgumentRecord,
        destination: &abstract_operations_to_target_operations::calling_conventions::ValuePlacement,
    ) -> bool {
        let CallInputs {
            target,
            function,
            function_bytes,
            custody,
            ..
        } = self.inputs;
        let CallSpan { end, .. } = self.span;
        let StackFacts {
            expected_call_stack_bytes,
            ..
        } = self.stacks;
        let Some(source_placement) = argument.source.placement() else {
            return true;
        };
        let sources = self.argument_sources(argument, source_placement);
        let ArgumentSources {
            parameter_source,
            result_source,
            local_source,
            affine_scalar_record_source,
        } = sources;
        let zero_byte_argument = (parameter_source || local_source)
            && argument.path.is_empty()
            && argument.byte_count == 0
            && argument.bytes.is_empty()
            && argument.shape == abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(0, 1)
            && source_placement.locations.is_empty()
            && argument.destination.locations.is_empty();
        argument.destination != *destination
            || argument.call_stack_bytes != expected_call_stack_bytes
            || (!parameter_source
                && !result_source
                && !local_source
                && !affine_scalar_record_source)
            || (argument.byte_count == 0 && !zero_byte_argument)
            || argument.bytes.len() != argument.byte_count
            || argument
                .code_offset
                .checked_add(argument.byte_count)
                .and_then(|end| function_bytes.get(argument.code_offset..end))
                != Some(argument.bytes.as_slice())
            || (!argument.path.is_empty()
                && expected_projected_copy_bytes(target, argument).as_deref()
                    != Some(argument.bytes.as_slice()))
            || (affine_scalar_record_source
                && expected_affine_scalar_record_argument_bytes(target, argument, function)
                    .as_deref()
                    != Some(argument.bytes.as_slice()))
            || argument.code_offset < custody.code_offset
            || argument
                .code_offset
                .checked_add(argument.byte_count)
                .is_none_or(|argument_end| argument_end > end)
            || argument
                .source_byte_offset
                .checked_add(u32::from(argument.shape.byte_size))
                .is_none_or(|end| end > u32::from(source_placement.shape.byte_size))
            || self.argument_path_is_malformed(argument_index, argument, source_placement, sources)
    }

    /// Where the argument's custody may come from: the parameter home it
    /// reads, the call's projected affine result, a trivial affine local
    /// established earlier and passed exactly once, or an affine scalar record
    /// established earlier and passed owned exactly once.
    fn argument_sources(
        &self,
        argument: &post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitCallArgumentRecord,
        source_placement: &abstract_operations_to_target_operations::calling_conventions::ValuePlacement,
    ) -> ArgumentSources {
        let CallInputs {
            function,
            provenance,
            internal_unit_calls,
            parameter_homes,
            custody,
            affine_cleanup,
            ..
        } = self.inputs;
        let StackFacts {
            operation_position, ..
        } = self.stacks;
        let ProjectionFacts {
            projected_result,
            projected_home,
            ..
        } = self.projection;
        let parameter_source = parameter_homes
            .iter()
            .find(|home| home.place == argument.place)
            .is_some_and(|home| {
                argument.root_structural_type == home.structural_type
                    && *source_placement == home.source
                    && source_placement.shape == home.shape
                    && home.location.stack_byte_offset().is_some_and(|offset| {
                        argument.source_location.stack_byte_offset() == Some(offset)
                    })
                    && (argument.path.is_empty()
                        || projected_home.is_some_and(|projected| {
                            projected.place == home.place
                                && projected.structural_type == home.structural_type
                        }))
            });
        let result_source = projected_result.zip(affine_cleanup).is_some_and(|(result, cleanup)| {
        crate::image_emission::object_artifact::replay::structural::affine_projected_calls::exact_owned_result_projection(
            argument, result, &cleanup.structural_types)
    });
        let local_source = affine_cleanup
            .and_then(|cleanup| {
                cleanup.locals.iter().find(|(_, place, structural_type)| {
                    place.id == argument.place
                        && argument.path.is_empty()
                        && argument.access == terminal_psi::StructuralAccess::Owned
                        && argument.root_structural_type == structural_type.id
                        && argument.structural_type == structural_type.id
                        && argument.shape == abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(0, 1)
                        && source_placement.shape == argument.shape
                        && source_placement.locations.is_empty()
                        && argument.destination.shape == argument.shape
                        && argument.destination.locations.is_empty()
                        && argument.source_location.stack_byte_offset() == Some(0)
                        && matches!(
                            place.kind,
                            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                                structural_type: local_type,
                                construction: None,
                                ..
                            } if local_type == structural_type.id
                        )
                        && matches!(
                            structural_type.shape,
                            terminal_psi::StructuralTypeShape::Record { ref fields }
                                if fields.is_empty()
                        )
                })
            })
            .is_some_and(|(establishment, _, _)| {
                provenance
                    .operations
                    .iter()
                    .position(|candidate| candidate == establishment)
                    .is_some_and(|position| position < operation_position)
                    && internal_unit_calls
                        .iter()
                        .flat_map(|call| &call.arguments)
                        .filter(|candidate| {
                            candidate.place == argument.place && candidate.path.is_empty()
                        })
                        .count()
                        == 1
            });
        let affine_scalar_record_source = function
            .unit_affine_scalar_records
            .iter()
            .find(|record| record.result.place == argument.place)
            .is_some_and(|record| {
                record.operation_ordinal < custody.operation_ordinal
                    && record.shape == ValueShape::integer(8, 8)
                    && record.result.structural_type == argument.structural_type
                    && argument.path.is_empty()
                    && argument.access == terminal_psi::StructuralAccess::Owned
                    && argument.root_structural_type == argument.structural_type
                    && argument.shape == record.shape
                    && source_placement.shape == record.shape
                    && source_placement.locations.is_empty()
                    && argument.source_location.stack_byte_offset() == Some(0)
                    && record.result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                    && record.result.qualifications.is_empty()
                    && record.result.projected_qualifications.is_empty()
                    && record.result.claims.is_empty()
                    && matches!(record.value, semantic_vocabulary::IntegerValue::Signed(value)
                    if i64::try_from(value).is_ok())
                    && provenance
                        .operations
                        .iter()
                        .position(|candidate| candidate == &record.psi_operation)
                        .is_some_and(|position| position < operation_position)
                    && internal_unit_calls
                        .iter()
                        .flat_map(|call| &call.arguments)
                        .filter(|candidate| {
                            candidate.place == argument.place
                                && candidate.path.is_empty()
                                && candidate.access == terminal_psi::StructuralAccess::Owned
                        })
                        .count()
                        == 1
            });
        ArgumentSources {
            parameter_source,
            result_source,
            local_source,
            affine_scalar_record_source,
        }
    }

    /// Whether the argument's path is malformed for its source: an unprojected
    /// argument reads its whole placement; a projected one is an exact
    /// borrowed or owned projection, a fixed-index element, a two-level
    /// fixed-index element, or a field path aligned inside the placement.
    fn argument_path_is_malformed(
        &self,
        argument_index: usize,
        argument: &post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitCallArgumentRecord,
        source_placement: &abstract_operations_to_target_operations::calling_conventions::ValuePlacement,
        sources: ArgumentSources,
    ) -> bool {
        let CallInputs {
            parameter_homes,
            affine_cleanup,
            ..
        } = self.inputs;
        let ArgumentSources { result_source, .. } = sources;
        match argument.path.as_slice() {
        [] => {
            argument.source_byte_offset != 0
                || source_placement.shape != argument.shape
                || argument.root_structural_type != argument.structural_type
                || argument.fixed_array_length.is_some()
                || argument.element_stride.is_some()
        }
        _ if self.exact_borrowed_argument(argument_index, argument) => false,
        _ if result_source => false,
        _ if argument.access == terminal_psi::StructuralAccess::Owned
            && parameter_homes.iter().any(|home| {
                home.place == argument.place
                    && home.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            }) =>
        {
            parameter_homes.iter().find(|home| home.place == argument.place)
                .zip(affine_cleanup)
                .is_none_or(|(home, cleanup)| {
                    !crate::image_emission::object_artifact::replay::structural::affine_projected_calls::exact_owned_projection(
                        argument, home, &cleanup.structural_types,
                    )
                })
        }
        [terminal_psi::StructuralPathSegment::FixedIndex(index)] => {
            let expected_stride = u32::from(argument.shape.byte_size)
                .next_multiple_of(u32::from(argument.shape.alignment));
            let Some(length) = argument.fixed_array_length else {
                return true;
            };
            let Some(stride) = argument.element_stride else {
                return true;
            };
            argument.root_structural_type == argument.structural_type
                || *index >= length
                || stride != expected_stride
                || u64::from(stride).checked_mul(*index)
                    != Some(u64::from(argument.source_byte_offset))
                || u64::from(stride).checked_mul(length)
                    != Some(u64::from(source_placement.shape.byte_size))
                || source_placement.shape.alignment != argument.shape.alignment
        }
        [
            terminal_psi::StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
            terminal_psi::StructuralPathSegment::FixedIndex(
                inner @ (0..=15),
            ),
        ] => {
            let leaf_stride = u32::from(argument.shape.byte_size)
                .next_multiple_of(u32::from(argument.shape.alignment));
            let Some(outer_stride) = argument.element_stride else {
                return true;
            };
            let Some(inner_length) =
                [
                    3_u32, 4_u32, 5_u32, 6_u32, 7_u32, 8_u32, 9_u32, 10_u32,
                    11_u32, 12_u32, 13_u32, 14_u32, 15_u32, 16_u32,
                ]
                    .into_iter()
                    .find(|length| {
                        leaf_stride.checked_mul(*length) == Some(outer_stride)
                    })
            else {
                return true;
            };
            let expected_offset = outer_stride
                .checked_mul(u32::try_from(*outer).unwrap_or(u32::MAX))
                .and_then(|offset| {
                    leaf_stride
                        .checked_mul(u32::try_from(*inner).unwrap_or(u32::MAX))
                        .and_then(|inner| offset.checked_add(inner))
                });
            argument.root_structural_type == argument.structural_type
                || argument.fixed_array_length != Some(2)
                || *inner >= u64::from(inner_length)
                || Some(argument.source_byte_offset) != expected_offset
                || outer_stride.checked_mul(2)
                    != Some(u32::from(source_placement.shape.byte_size))
                || source_placement.shape.alignment != argument.shape.alignment
        }
        path @ [terminal_psi::StructuralPathSegment::Field(_), ..]
            if path.iter().all(|segment| {
                matches!(segment,
                    terminal_psi::StructuralPathSegment::Field(identity)
                        if !identity.is_empty())
            }) =>
        {
            path.is_empty()
                || argument.root_structural_type == argument.structural_type
                || argument.fixed_array_length.is_some()
                || argument.element_stride.is_some()
                || !argument
                    .source_byte_offset
                    .is_multiple_of(u32::from(argument.shape.alignment))
        }
        _ => true,
    }
    }

    /// Whether a projected argument is left unsettled: it is neither a claim
    /// transfer nor an exact borrowed projection, and no fully consumed
    /// parameter, projected result or disjoint residual discard accounts for
    /// it.
    pub(super) fn projected_arguments_are_unsettled(&self) -> bool {
        let CallInputs {
            custody,
            affine_cleanup,
            fully_consumed_affine_parameter,
            ..
        } = self.inputs;
        let ProjectionFacts {
            projected_result, ..
        } = self.projection;
        let projected_argument_indexes = &self.projection.projected_argument_indexes;
        let transferred_argument_indexes = &self.projection.transferred_argument_indexes;
        projected_argument_indexes.iter().any(|index| {
            if transferred_argument_indexes.contains(index) {
                return false;
            }
            let Some(argument) = custody.arguments.get(*index) else {
                return true;
            };
            if self.exact_borrowed_argument(*index, argument) {
                return false;
            }
            argument.path.is_empty()
                || (!fully_consumed_affine_parameter
                    && projected_result.is_none()
                    && affine_cleanup.is_none_or(|cleanup| {
                        !cleanup.actions.iter().any(|action| {
                            matches!(action,
                        terminal_psi::TerminalAffineCleanupAction::DiscardResidual(residual)
                            if residual.place == argument.place
                                && !residual.path.is_empty()
                                && !residual.path.starts_with(&argument.path)
                                && !argument.path.starts_with(&residual.path)
                                && residual.structural_type
                                    != argument.root_structural_type)
                        })
                    }))
        })
    }

    /// Whether the claim transfers are malformed: one names an argument
    /// outside the roster, or two transfer the same claim.
    pub(super) fn claim_transfers_are_malformed(&self) -> bool {
        let CallInputs { custody, .. } = self.inputs;
        custody.claim_transfers.iter().any(|transfer| {
            usize::try_from(transfer.argument_index)
                .map_or(true, |index| index >= custody.arguments.len())
        }) || custody
            .claim_transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != custody.claim_transfers.len()
    }
}

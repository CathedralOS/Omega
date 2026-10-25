//! A branch state's non-scalar effects consume completed operands without
//! adding value slots.
use super::{LoweringError, ValueDeclaration, emit_direct_expression, unsupported};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::view_subslice;
use crate::scalar_lowering::branch_states::prepared::LoweredScalarEffect;

pub(crate) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    environment: &mut crate::unit::attached_unit::OperationEnvironment<'_>,
    effects: &[LoweredScalarEffect],
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
    span: &mut crate::unit::attached_unit::argument_evaluation::BlockEmissionSpan,
    blocks: &mut Vec<terminal_psi::Block>,
    next_block: &mut u64,
    next_edge: &mut u64,
    erased_proof_formals: &[checked_trees::CheckedErasedProofParameterPlan],
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
) -> Result<(), LoweringError> {
    let checked = environment.checked;
    for effect in effects {
        let first_operation = operations.len();
        match effect {
            LoweredScalarEffect::ObserveStructuralValue { value, observation } => {
                let completed = operations
                    .computed_values
                    .iter()
                    .rev()
                    .find(|(candidate, _)| candidate == value)
                    .map(|(_, binding)| binding.clone())
                    .ok_or(LoweringError::Unsupported(
                        "computed observation has no completed value",
                    ))?;
                if checked
                    .facts
                    .values
                    .structural_values
                    .root_for_value(*value)
                    .is_some_and(|root| root.captured_copy.is_valid())
                {
                    evaluation.saved_value_binding(*value, operations)?;
                }
                let node = checked
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(*observation);
                let mut expression = match &node.kind {
                    checked_trees::CheckedScalarComputationKind::StructuralField {
                        receiver_value,
                        subject,
                        field,
                        atomic_ordering,
                        ..
                    } => crate::scalar_lowering::computations::fields::emitted_observation(
                        identities,
                        checked,
                        &evaluation.record_fields,
                        &evaluation.contextual_bindings(),
                        calls.structural_types,
                        *value,
                        *receiver_value,
                        subject,
                        *field,
                        node.primitive_type,
                        *atomic_ordering,
                        &completed,
                    )?,
                    checked_trees::CheckedScalarComputationKind::CaseMembership {
                        case,
                        subject,
                        ..
                    } => {
                        if !terminal_psi::StructuralAccess::SharedBorrow
                            .admits(completed.argument.access)
                        {
                            return unsupported(
                                "case observation requires readable completed custody",
                            );
                        }
                        let case =
                            crate::scalar_lowering::computations::cases::structural_case_identity(
                                checked, *case,
                            )?;
                        let checked_trees::CheckedScalarComputationStructuralArgument::Place(
                            subject,
                        ) = subject
                        else {
                            return unsupported("completed case observation has no source path");
                        };
                        let (path, case) =
                            crate::expression_preparation::bindings::structural_cases::resolve_observation_at(
                                identities,
                                calls.structural_types,
                                completed.structural_type,
                                &subject.path,
                                &case,
                            )?;
                        let mut prefix = completed.argument.path.clone();
                        prefix.extend(path);
                        crate::emission::operation_emission::expressions::LoweredDirectExpression::Boolean {
                            expression: Box::new(crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression::StructuralCaseMembership {
                                source: completed.argument.place, path: prefix, case,
                            }),
                        }
                    }
                    _ => {
                        return unsupported(
                            "retained value observation has no structural observation",
                        );
                    }
                };
                let types = values
                    .iter()
                    .map(|value| value.value_type())
                    .collect::<Vec<_>>();
                if let checked_trees::CheckedScalarComputationKind::StructuralField {
                    result_type,
                    ..
                } = &node.kind
                {
                    expression.retain_observed_type(
                        evaluation
                            .shared_qualifications
                            .value_type(checked, *result_type)?,
                        &types,
                    )?;
                }
                let value_type = expression.value_type(&types)?;
                let id = emit_direct_expression(&expression, values, next_value, operations);
                values.push(ValueDeclaration {
                    id,
                    scalar_type: value_type.scalar_type,
                    qualifications: value_type.qualifications,
                });
            }
            LoweredScalarEffect::CompleteValueObservation { .. }
            | LoweredScalarEffect::CompleteObservationReceiver { .. }
            | LoweredScalarEffect::CompleteTemporaryOperands { .. } => {
                let control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
                let actions = match effect {
                    LoweredScalarEffect::CompleteObservationReceiver { actions } => checked
                        .facts
                        .values
                        .scalar_computations
                        .temporary_cleanup_actions
                        .span(*actions)
                        .ok_or(LoweringError::Unsupported(
                            "receiver observation lost its checked action roster",
                        ))?
                        .iter()
                        .collect::<Vec<_>>(),
                    LoweredScalarEffect::CompleteValueObservation {
                        machine,
                        state,
                        statement,
                        expression,
                    } => {
                        let completion = control
                            .value_completion(*machine, *state, *statement, *expression)
                            .ok_or(LoweringError::Unsupported(
                                "observation completion lost its checked disposition",
                            ))?;
                        control
                            .actions
                            .span_or_empty(completion.actions)
                            .iter()
                            .collect::<Vec<_>>()
                    }
                    LoweredScalarEffect::CompleteTemporaryOperands { cleanups } => {
                        let plans = &checked.facts.values.scalar_computations;
                        let rows = plans.temporary_cleanups.span(*cleanups).ok_or(
                            LoweringError::Unsupported(
                                "call completion lost its checked operand roster",
                            ),
                        )?;
                        let mut actions = Vec::new();
                        for row in rows {
                            let selected = plans
                                .temporary_cleanup_actions
                                .span(row.actions)
                                .ok_or(LoweringError::Unsupported(
                                    "call completion lost its checked action roster",
                                ))?;
                            if selected.is_empty() {
                                return unsupported(
                                    "call completion has no selected owner disposal",
                                );
                            }
                            actions.extend(selected);
                        }
                        actions
                    }
                    _ => return unsupported("expression cleanup received a non-completion effect"),
                };
                let regions = crate::unit::unit_cleanup::expression_cleanup_regions(
                    identities,
                    checked,
                    actions,
                    &evaluation.structural_locals,
                    |place| {
                        // Expression owners are completed private producers or
                        // arrivals, never a source-type multiplicity guess.
                        let mut multiplicities = operations
                            .iter()
                            .filter_map(|operation| operation.result.structural())
                            .filter(|result| result.place == place)
                            .map(|result| result.multiplicity)
                            .chain(
                                span.structural_parameters
                                    .iter()
                                    .chain(
                                        blocks
                                            .iter()
                                            .flat_map(|block| &block.structural_parameters),
                                    )
                                    .filter(|parameter| parameter.place == place)
                                    .map(|parameter| parameter.multiplicity),
                            );
                        let multiplicity =
                            multiplicities.next().ok_or(LoweringError::Unsupported(
                                "value completion has no emitted owner carrier",
                            ))?;
                        if multiplicities.any(|other| other != multiplicity) {
                            return unsupported(
                                "value completion has inconsistent emitted carriers",
                            );
                        }
                        Ok(multiplicity)
                    },
                    calls,
                )?;
                crate::unit::attached_unit::argument_evaluation::cleanup_control::CleanupEmission {
                    structural_types: calls.structural_types,
                    source_parameters: &evaluation.structural_parameters,
                    previous_parameter_homes: &evaluation.previous_parameter_homes,
                    checked,
                    current: &mut span.id,
                    parameters: &mut span.parameters,
                    structural_parameters: &mut span.structural_parameters,
                    block_erased_scalar_formals: &mut span.erased_scalar_formals,
                    erased_proof_formals,
                    operation_start: &mut span.operation_start,
                    blocks,
                }
                .emit(
                    identities, &regions, values, next_value, next_block, next_edge, operations,
                )?;
                for region in &regions {
                    if region.action.path().is_empty() {
                        evaluation.retire_structural_home(region.action.place());
                    }
                }
            }
            LoweredScalarEffect::CompleteArgumentSnapshots { places } => {
                let mut regions = Vec::new();
                for place in places.iter().rev() {
                    regions.extend(evaluation.normal_discard_regions(
                        *place,
                        calls.structural_types,
                        operations,
                    )?);
                }
                crate::unit::attached_unit::argument_evaluation::cleanup_control::CleanupEmission {
                    structural_types: calls.structural_types,
                    source_parameters: &evaluation.structural_parameters,
                    previous_parameter_homes: &evaluation.previous_parameter_homes,
                    checked,
                    current: &mut span.id,
                    parameters: &mut span.parameters,
                    structural_parameters: &mut span.structural_parameters,
                    block_erased_scalar_formals: &mut span.erased_scalar_formals,
                    erased_proof_formals,
                    operation_start: &mut span.operation_start,
                    blocks,
                }
                .emit(
                    identities, &regions, values, next_value, next_block, next_edge, operations,
                )?;
                for place in places {
                    evaluation.retire_structural_home(*place);
                }
            }
            LoweredScalarEffect::CaptureStructuralPlace {
                source,
                place,
                capture,
                access,
            } => {
                let mut source = source.emit(values, calls, next_value, operations)?;
                source.access = *access;
                let parameters = environment
                    .parameters
                    .iter()
                    .chain(span.structural_parameters.iter())
                    .cloned()
                    .collect::<Vec<_>>();
                let (qualifications, projected) = crate::unit::attached_unit::argument_memberships(
                    &source,
                    &parameters,
                    evaluation,
                    operations,
                )?;
                let (structural_type, qualifications, projected) =
                    crate::emission::completed_place::present_capture(
                        capture,
                        *access,
                        calls.structural_types,
                        calls.domain_declarations,
                        qualifications,
                        projected,
                    )?;
                evaluation.current = span.id;
                evaluation.operation_start = span.operation_start;
                std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                std::mem::swap(
                    &mut evaluation.block_structural_parameters,
                    &mut span.structural_parameters,
                );
                std::mem::swap(&mut evaluation.blocks, blocks);
                let captured = crate::emission::completed_address::capture_into(
                    source.place,
                    source.path,
                    structural_type,
                    *access,
                    qualifications,
                    projected,
                    *place,
                    evaluation,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    environment.private_places,
                    operations,
                );
                span.id = evaluation.current;
                span.operation_start = evaluation.operation_start;
                std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                std::mem::swap(
                    &mut evaluation.block_structural_parameters,
                    &mut span.structural_parameters,
                );
                std::mem::swap(&mut evaluation.blocks, blocks);
                captured?;
            }
            LoweredScalarEffect::CopyStructuralValue {
                source,
                place,
                structural_type,
            } => {
                let source = source.emit(values, calls, next_value, operations)?;
                let parameters = environment
                    .parameters
                    .iter()
                    .chain(span.structural_parameters.iter())
                    .cloned()
                    .collect::<Vec<_>>();
                let declaration = crate::unit::attached_unit::emit_argument_snapshot(
                    &source,
                    *place,
                    *structural_type,
                    &parameters,
                    evaluation,
                    operations,
                    calls.structural_types,
                    calls.domain_declarations,
                )?;
                environment.private_places.push(declaration);
            }

            LoweredScalarEffect::ProduceStructuralValue {
                value,
                structural_type,
            } => {
                // The scalar graph owns its active span. Lend that same span
                // to ordinary value emission, then take its actual continuation
                // back; no second buffer or empty source environment is built.
                evaluation.current = span.id;
                evaluation.operation_start = span.operation_start;
                std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                std::mem::swap(
                    &mut evaluation.block_structural_parameters,
                    &mut span.structural_parameters,
                );
                std::mem::swap(&mut evaluation.blocks, blocks);
                let produced = environment.produce_value(
                    identities,
                    *value,
                    *structural_type,
                    evaluation,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    calls,
                    operations,
                );
                span.id = evaluation.current;
                span.operation_start = evaluation.operation_start;
                std::mem::swap(&mut evaluation.parameters, &mut span.parameters);
                std::mem::swap(
                    &mut evaluation.block_structural_parameters,
                    &mut span.structural_parameters,
                );
                std::mem::swap(&mut evaluation.blocks, blocks);
                produced?;
            }
            LoweredScalarEffect::BindCompletedValueAlias { value, alias } => {
                evaluation.bind_completed_value_alias(*value, *alias, operations)?;
            }
            LoweredScalarEffect::EstablishScalarCase(case) => {
                crate::scalar_lowering::computations::cases::emit(
                    case, values, next_value, operations, calls,
                )?
            }
            LoweredScalarEffect::EstablishByteSequenceLiteral {
                literal,
                memberships,
            } => {
                let qualifications = memberships
                    .iter()
                    .map(|domain| {
                        crate::terminal_identities::lookup_domain_id_for_carrier(
                            calls.domain_ids,
                            *domain,
                            literal.structural_type,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                literal.emit(
                    qualifications,
                    next_value,
                    &mut calls.next_obligation_identity,
                    operations,
                )?;
            }
            LoweredScalarEffect::EstablishElementView(view) => {
                crate::unit::attached_unit::view_ranges::establish(
                    view, values, next_value, operations,
                )?;
            }
            LoweredScalarEffect::ViewSubslice {
                source,
                start,
                end,
                place,
                structural_type,
            } => {
                let path = crate::emission::operation_emission::expressions::emit_prepared_path(
                    &source.path,
                    0,
                    values,
                    next_value,
                    operations,
                );
                view_subslice::emit(
                    source.family,
                    crate::expression_preparation::bindings::structural_paths::structural_argument(
                        source.place,
                        path,
                        source.access,
                    ),
                    Some(start),
                    end.as_ref(),
                    *place,
                    *structural_type,
                    values,
                    next_value,
                    operations,
                )?;
            }
        }
        span.current_claims
            .publish_operations(&operations.operations[first_operation..])?;
    }
    Ok(())
}

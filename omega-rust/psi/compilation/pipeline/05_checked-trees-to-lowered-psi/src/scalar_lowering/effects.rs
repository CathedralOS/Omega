//! A branch state's non-scalar effects consume completed operands without
//! adding value slots.
use super::{
    LoweringError, OperationKind, OperationResult, ValueDeclaration, emit_direct_expression,
    unsupported,
};
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
                            crate::expression_preparation::bindings::structural_cases::resolve_at(
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
                    evaluation.retire_structural_home(region.action.place());
                }
            }
            LoweredScalarEffect::CopyStructuralValue {
                source,
                place,
                structural_type,
                multiplicity,
            } => {
                use crate::expression_preparation::bindings::PreparedCopyPathSegment;
                let mut path = Vec::with_capacity(source.path.len());
                for segment in &source.path {
                    path.push(match segment {
                        PreparedCopyPathSegment::Static(segment) => segment.clone(),
                        PreparedCopyPathSegment::Index(expression) => {
                            if !matches!(expression.scalar_type(), semantic_vocabulary::ScalarType::Integer(integer) if !integer.is_address()) {
                                return unsupported("copied value selector is not an integer");
                            }
                            let index = emit_direct_expression(expression, values, next_value, operations);
                            terminal_psi::StructuralPathSegment::RuntimeIndex {
                                index,
                                obligation: calls.allocate_requirement()?,
                            }
                        }
                    });
                }
                if let Some(length) = &source.length_observation {
                    emit_direct_expression(length, values, next_value, operations);
                }
                // Read the source's published qualifications, never the copy
                // destination's requested memberships. Projected copies retain
                // only the exact selected subtree's qualification paths.
                let (whole, projected) = operations
                    .iter()
                    .filter_map(|operation| operation.result.structural())
                    .find(|result| result.place == source.place)
                    .map(|result| (&result.qualifications, &result.projected_qualifications))
                    .unwrap_or((&source.qualifications, &source.projected_qualifications));
                let (mut qualifications, projected_qualifications) =
                    terminal_psi::StructuralPathQualification::select(whole, projected, &path);
                for domain in &source.formation_qualifications {
                    qualifications.push(crate::terminal_identities::lookup_domain_id_for_carrier(
                        calls.domain_ids,
                        *domain,
                        *structural_type,
                    )?);
                }
                qualifications.sort();
                qualifications.dedup();
                operations.push_leaf(
                    OperationResult::Structural(terminal_psi::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: *place,
                        structural_type: *structural_type,
                        multiplicity: *multiplicity,
                        qualifications,
                        projected_qualifications,
                        claims: Vec::new(),
                    }),
                    OperationKind::StructuralLeafCopy {
                        source: source.place,
                        path,
                    },
                );
            }

            LoweredScalarEffect::ProduceStructuralValue {
                value,
                structural_type,
                alias,
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
    }
    Ok(())
}

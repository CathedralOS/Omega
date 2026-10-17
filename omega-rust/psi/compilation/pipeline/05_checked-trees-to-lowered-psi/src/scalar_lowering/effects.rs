//! A branch state's non-scalar effects consume completed operands without
//! adding value slots.
use super::{
    LoweringError, OperationKind, OperationResult, ValueDeclaration, emit_direct_expression,
    lookup_machine_id, unsupported, validate_direct_parameter_types,
};
use crate::direct_calls::DirectCallResult;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::view_subslice;
use crate::scalar_lowering::branch_states::prepared::{EffectCallTarget, LoweredScalarEffect};

pub(crate) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    effects: &[LoweredScalarEffect],
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
    span: &mut crate::unit::attached_unit::argument_evaluation::BlockEmissionSpan,
    blocks: &mut Vec<terminal_psi::Block>,
    next_block: &mut u64,
    next_edge: &mut u64,
) -> Result<(), LoweringError> {
    for effect in effects {
        match effect {
            LoweredScalarEffect::QuotientCall {
                value,
                arguments,
                place,
                structural_type,
            } => {
                crate::emission::quotient_values::emit_call(
                    checked,
                    *value,
                    arguments,
                    *place,
                    *structural_type,
                    next_value,
                    operations,
                    calls,
                )?;
            }
            LoweredScalarEffect::QuotientAdaptation {
                value,
                source,
                source_type,
                place,
                structural_type,
            } => {
                crate::emission::quotient_values::emit_adaptation(
                    *value,
                    *source,
                    *source_type,
                    *place,
                    *structural_type,
                    operations,
                );
            }
            LoweredScalarEffect::EstablishReference {
                storage,
                backing_type,
                qualification_decisions,
                state,
                statement,
                expression,
                place,
                structural_type,
                source,
            } => {
                let source = source.emit(values, calls, next_value, operations)?;
                let (storage, _) = crate::unit::attached_unit::reference_results::publish_storage(
                    *storage, next_value, operations,
                )?;
                crate::unit::attached_unit::reference_results::emit_carrier(
                    crate::unit::attached_unit::reference_results::Formation {
                        checked,
                        state: *state,
                        statement_index: *statement as usize,
                        expression: *expression,
                        backing_type: *backing_type,
                        qualification_decisions: *qualification_decisions,
                        domain_ids: calls.domain_ids,
                    },
                    *structural_type,
                    source,
                    storage,
                    calls.structural_types,
                    std::iter::empty(),
                    *place,
                    operations,
                )?;
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

            LoweredScalarEffect::EstablishAggregate(aggregate) => {
                crate::scalar_lowering::branch_states::structural_values::emit(
                    aggregate, values, next_value, operations, calls,
                )?;
            }
            LoweredScalarEffect::EstablishZeroedStructural {
                memberships,
                projected_qualifications,
                place,
                structural_type,
                multiplicity,
                claims,
            } => {
                operations.push_leaf(
                    OperationResult::Structural(super::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: *place,
                        structural_type: *structural_type,
                        multiplicity: *multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications:
                            crate::unit::attached_unit::parameters::lower_projected_qualifications(
                                identities,
                                projected_qualifications,
                                calls.domain_ids,
                                *structural_type,
                                calls.structural_types,
                            )?,
                        claims: crate::emission::claim_bindings::emit_establishment_claims(
                            claims,
                            &mut calls.claims,
                        )?,
                    }),
                    OperationKind::EstablishZeroedStructural {},
                );
                let domains = memberships
                    .iter()
                    .map(|domain| {
                        crate::terminal_identities::lookup_domain_id_for_carrier(
                            calls.domain_ids,
                            *domain,
                            *structural_type,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                crate::emission::formation_memberships::publish(
                    operations,
                    *place,
                    *structural_type,
                    domains,
                )?;
                crate::emission::claim_bindings::record_prepared_introductions(
                    operations,
                    *place,
                    claims,
                    &calls.claims,
                    crate::emission::claim_bindings::ClaimEstablishmentKind::Construction,
                )?;
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
            LoweredScalarEffect::EstablishScalarHome {
                place,
                structural_type,
                value,
            } => {
                use crate::unit::attached_unit::primitive_locals;
                let initialized = emit_direct_expression(value, values, next_value, operations);
                let obligation = primitive_locals::initialization_obligation(
                    calls.structural_types,
                    *structural_type,
                    &mut calls.next_obligation_identity,
                )?;
                primitive_locals::establish_at(
                    *place,
                    operations,
                    initialized,
                    *structural_type,
                    calls.structural_types,
                    obligation,
                )?;
            }
            LoweredScalarEffect::EstablishScalarArray(array) => {
                crate::scalar_lowering::computations::arrays::emit(
                    std::slice::from_ref(array),
                    calls.domain_ids,
                    values,
                    next_value,
                    operations,
                )?
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
            LoweredScalarEffect::Call(call) => {
                let types = values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>();
                let value_types = values
                    .iter()
                    .map(|value| value.value_type())
                    .collect::<Vec<_>>();
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| {
                        validate_direct_parameter_types(argument, &types)?;
                        if !matches!(argument, LoweredDirectExpression::Parameter { .. }) {
                            return unsupported("effect call requires completed scalar operands");
                        }
                        let id = emit_direct_expression(argument, values, next_value, operations);
                        let value_type = argument.value_type(&value_types)?;
                        Ok(ValueDeclaration {
                            qualifications: value_type.qualifications,
                            id,
                            scalar_type: value_type.scalar_type,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                // Only dispatch changes the operation kind. The common result
                // receiver below binds declared qualifications and actual claims.
                let kind = match &call.endpoint {
                    EffectCallTarget::Dynamic {
                        interface_identity,
                        requirement_identity,
                        family_actuals,
                        family_results,
                    } => {
                        let structural_arguments = call
                            .structural_arguments
                            .iter()
                            .map(|argument| argument.emit(values, calls, next_value, operations))
                            .collect::<Result<Vec<_>, _>>()?;
                        let claim_transfers =
                            crate::emission::claim_bindings::emit_prepared_claim_transfers(
                                &call.claim_transfers,
                                calls.claims.bindings(),
                            )?;
                        let family_actuals = family_actuals
                            .iter()
                            .map(|actual| {
                                validate_direct_parameter_types(actual, &types)?;
                                let value_type = actual.value_type(&value_types)?;
                                Ok(ValueDeclaration {
                                    id: emit_direct_expression(
                                        actual, values, next_value, operations,
                                    ),
                                    scalar_type: value_type.scalar_type,
                                    qualifications: value_type.qualifications,
                                })
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()?;
                        let result = call
                            .result
                            .as_ref()
                            .map(|result| {
                                result.emit(
                                    identities,
                                    checked,
                                    call.source_coordinate.state,
                                    call.checked_coordinate,
                                    calls.domain_ids,
                                    calls.structural_types,
                                    &mut calls.claims,
                                )
                            })
                            .transpose()?
                            .map(OperationResult::Structural)
                            .unwrap_or(OperationResult::Unit);
                        let operation = operations.allocate();
                        operations.record_source_call_with_values(
                            call.source_coordinate,
                            call.source_site,
                            operation,
                            call.target_machine,
                            values,
                        )?;
                        crate::emission::operation_emission::dynamic_calls::emit_completed_dynamic_call(
                            interface_identity, requirement_identity, &arguments, &family_actuals,
                            structural_arguments, claim_transfers, &result, operation, span, blocks,
                            next_value, next_block, next_edge, operations,
                            |tuple| family_results.get(tuple).copied().ok_or(LoweringError::Unsupported(
                                "dynamic family result has no reserved leaf home",
                            )),
                        )?;
                        crate::emission::loan_completions::emit(
                            &call.loan_completions,
                            call.source_coordinate,
                            operations,
                            calls,
                        )?;
                        continue;
                    }
                    EffectCallTarget::Boundary => {
                        // Each endpoint prepares these completed operands once.
                        // Module preparation owns its structural lanes below;
                        // emitting them here too would duplicate indexed-path
                        // operations and their exact proof obligations.
                        let structural_arguments = call
                            .structural_arguments
                            .iter()
                            .map(|argument| argument.emit(values, calls, next_value, operations))
                            .collect::<Result<Vec<_>, _>>()?;
                        let target = calls
                            .boundaries
                            .iter()
                            .find(|target| target.source == call.target_machine)
                            .ok_or(LoweringError::Unsupported(
                                "expression boundary call has no allocated declaration",
                            ))?;
                        crate::emission::operation_emission::boundary_calls::complete_operands(
                            target,
                            &arguments,
                            structural_arguments,
                            &call.claim_transfers,
                            calls,
                        )?
                        .into_kind()
                    }
                    EffectCallTarget::Module {
                        erased_proof_arguments,
                        crash_routes,
                    } => {
                        let prepared =
                            crate::unit::attached_unit::ordinary_calls::prepare_computed_module_call(
                                identities,
                                checked,
                                call.source_coordinate,
                                call.target_machine,
                                &arguments,
                                Vec::new(),
                                erased_proof_arguments,
                                &call.structural_arguments,
                                &call.claim_transfers,
                                crash_routes,
                                values,
                                erased,
                                calls,
                                next_value,
                                operations,
                            )?;
                        let result = call
                            .result
                            .as_ref()
                            .map(|result| {
                                result.emit(
                                    identities,
                                    checked,
                                    call.source_coordinate.state,
                                    call.checked_coordinate,
                                    calls.domain_ids,
                                    calls.structural_types,
                                    &mut calls.claims,
                                )
                            })
                            .transpose()?;
                        let result_kind =
                            result.as_ref().map_or(DirectCallResult::Unit, |result| {
                                DirectCallResult::Structural {
                                    multiplicity: result.multiplicity,
                                }
                            });
                        prepared.emit(
                            operations,
                            call.source_coordinate,
                            call.source_site,
                            call.target_state,
                            crate::direct_calls::CallRecord::Values(values),
                            result
                                .map(OperationResult::Structural)
                                .unwrap_or(OperationResult::Unit),
                            lookup_machine_id(calls.machine_ids, call.target_machine)?,
                            result_kind,
                        )?;
                        crate::emission::loan_completions::emit(
                            &call.loan_completions,
                            call.source_coordinate,
                            operations,
                            calls,
                        )?;
                        continue;
                    }
                };
                let result = call
                    .result
                    .as_ref()
                    .map(|result| {
                        result.emit(
                            identities,
                            checked,
                            call.source_coordinate.state,
                            call.checked_coordinate,
                            calls.domain_ids,
                            calls.structural_types,
                            &mut calls.claims,
                        )
                    })
                    .transpose()?;
                let operation = operations.allocate();
                operations.record_source_call_with_values(
                    call.source_coordinate,
                    call.source_site,
                    operation,
                    call.target_machine,
                    values,
                )?;
                operations.push(terminal_psi::Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: operation,
                    result: result
                        .map(OperationResult::Structural)
                        .unwrap_or(OperationResult::Unit),
                    kind,
                });
                crate::emission::loan_completions::emit(
                    &call.loan_completions,
                    call.source_coordinate,
                    operations,
                    calls,
                )?;
            }
        }
    }
    Ok(())
}

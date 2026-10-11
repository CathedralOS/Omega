//! A branch state's non-scalar effects consume completed operands without
//! adding value slots.
use super::{
    LoweringError, OperationKind, OperationResult, ValueDeclaration, emit_direct_expression,
    lookup_machine_id, lower_checked_crash_route_buckets, unsupported,
    validate_direct_parameter_types,
};
use crate::direct_calls::{DirectCall, DirectCallResult};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::view_subslice;
use crate::scalar_lowering::branch_states::prepared::LoweredScalarEffect;

pub(crate) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    effects: &[LoweredScalarEffect],
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<(), LoweringError> {
    for effect in effects {
        match effect {
            LoweredScalarEffect::EstablishReference {
                state,
                statement,
                expression,
                place,
                structural_type,
                source,
            } => {
                crate::unit::attached_unit::reference_results::emit_carrier(
                    crate::unit::attached_unit::reference_results::Formation {
                        checked,
                        state: *state,
                        statement_index: *statement as usize,
                        expression: *expression,
                    },
                    *structural_type,
                    source.clone(),
                    None,
                    *place,
                    operations,
                )?;
            }
            LoweredScalarEffect::CopyStructuralValue {
                source,
                place,
                structural_type,
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
                        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
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
                        projected_qualifications: Vec::new(),
                        claims: crate::emission::claim_bindings::emit_establishment_claims(
                            claims,
                            &mut calls.claims,
                        )?,
                    }),
                    OperationKind::EstablishZeroedStructural {},
                );
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
            LoweredScalarEffect::EstablishByteSequenceLiteral(literal) => {
                literal.emit(
                    Vec::new(),
                    next_value,
                    &mut calls.next_obligation_identity,
                    operations,
                )?;
            }
            LoweredScalarEffect::EstablishScalarArray(array) => {
                crate::scalar_lowering::computations::arrays::emit(
                    std::slice::from_ref(array),
                    values,
                    next_value,
                    operations,
                )?
            }
            LoweredScalarEffect::EstablishElementView(view) => {
                crate::unit::attached_unit::view_ranges::establish(
                    view, values, next_value, operations,
                );
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
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| {
                        validate_direct_parameter_types(argument, &types)?;
                        if !matches!(argument, LoweredDirectExpression::Parameter { .. }) {
                            return unsupported("effect call requires completed scalar operands");
                        }
                        let id = emit_direct_expression(argument, values, next_value, operations);
                        Ok(ValueDeclaration {
                            qualifications: Default::default(),
                            id,
                            scalar_type: argument.scalar_type(),
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let erased_proof_arguments = call
                    .erased_proof_arguments
                    .iter()
                    .map(|term| {
                        crate::scalar_lowering::contracts::lowered_proof_term(
                            checked, term, values, erased,
                        )
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let claim_transfers =
                    crate::emission::claim_bindings::emit_prepared_claim_transfers(
                        &call.claim_transfers,
                        calls.claims.bindings(),
                    )?;
                let mut result = call.result.clone();
                if let Some(result) = &mut result {
                    result.claims = crate::emission::claim_bindings::emit_call_result_claims(
                        &call.result_claims,
                        call.source_coordinate.state,
                        call.checked_coordinate,
                        &mut calls.claims,
                    )?;
                }
                crate::direct_calls::emit_direct_call(
                    operations,
                    call.source_coordinate,
                    None,
                    call.target_state,
                    crate::direct_calls::CallRecord::Values(values),
                    result
                        .map(OperationResult::Structural)
                        .unwrap_or(OperationResult::Unit),
                    DirectCall::new(
                        lookup_machine_id(calls.machine_ids, call.target_machine)?,
                        arguments.iter().map(|argument| argument.id).collect(),
                        Vec::new(),
                        erased_proof_arguments,
                        call.structural_arguments.clone(),
                        claim_transfers,
                        Vec::new(),
                        lower_checked_crash_route_buckets(
                            identities,
                            &call.crash_routes,
                            &arguments,
                        )?,
                        call.result
                            .as_ref()
                            .map_or(DirectCallResult::Unit, |result| {
                                DirectCallResult::Structural {
                                    multiplicity: result.multiplicity,
                                }
                            }),
                    ),
                )?;
            }
        }
    }
    Ok(())
}

//! Non-scalar graph effects consume completed operands without adding value slots.
use super::{
    LoweringError, Operation, OperationKind, OperationResult, ValueDeclaration,
    emit_direct_expression, lookup_machine_id, lower_checked_crash_route_buckets, unsupported,
    validate_direct_parameter_types,
};
use crate::direct_calls::{DirectCall, DirectCallResult};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::view_subslice;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarEffect;
use crate::terminal_identities::{allocate_dense, obligation_id};

pub(crate) fn emit(
    effects: &[LoweredScalarEffect],
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<(), LoweringError> {
    for effect in effects {
        match effect {
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
                if source.observe_length {
                    crate::emission::operation_emission::expressions::emit_element_length(
                        source.place,
                        next_value,
                        operations,
                    );
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
                let mut qualifications = if path.is_empty() {
                    whole.clone()
                } else {
                    Vec::new()
                };
                let mut projected_qualifications = Vec::new();
                for qualification in projected {
                    if let Some(suffix) = qualification.path.strip_prefix(path.as_slice()) {
                        if suffix.is_empty() {
                            qualifications.push(qualification.domain);
                        } else {
                            projected_qualifications.push(
                                terminal_psi::StructuralPathQualification {
                                    path: suffix.to_vec(),
                                    domain: qualification.domain,
                                },
                            );
                        }
                    }
                }
                qualifications.sort();
                qualifications.dedup();
                projected_qualifications.sort();
                projected_qualifications.dedup();
                let id = operations.allocate();
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: *place,
                        structural_type: *structural_type,
                        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                        qualifications,
                        projected_qualifications,
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::StructuralLeafCopy {
                        source: source.place,
                        path,
                    },
                });
            }
            LoweredScalarEffect::StoreScalarField {
                destination,
                path,
                field,
                value_position,
                scalar_type,
                requires_evidence,
                range_decided,
            } => {
                let value = values
                    .get(*value_position)
                    .filter(|value| value.scalar_type == *scalar_type)
                    .ok_or(LoweringError::Unsupported(
                        "record store lost its completed scalar operand",
                    ))?;
                let range_obligation = if *requires_evidence {
                    Some(obligation_id(allocate_dense(
                        &mut calls.next_obligation_identity,
                    )?))
                } else {
                    None
                };
                if *range_decided && let Some(obligation) = range_obligation {
                    operations.source_checked_obligations.push(obligation);
                }
                let id = operations.allocate();
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Unit,
                    kind: OperationKind::StructuralScalarFieldStore {
                        destination: *destination,
                        path: path.clone(),
                        field: *field,
                        value: value.id,
                        range_obligation,
                    },
                });
            }
            LoweredScalarEffect::EstablishAggregate(aggregate) => {
                crate::scalar_graph::scalar_graph_lowering::structural_values::emit(
                    aggregate, values, next_value, operations, calls,
                )?;
            }
            LoweredScalarEffect::EstablishZeroedStructural {
                place,
                structural_type,
                multiplicity,
            } => {
                let id = operations.allocate();
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Structural(super::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: *place,
                        structural_type: *structural_type,
                        multiplicity: *multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::EstablishZeroedStructural {},
                });
            }
            LoweredScalarEffect::EstablishScalarCase(case) => {
                crate::scalar_graph::scalar_computations::cases::emit(
                    case, values, next_value, operations, calls,
                )?
            }
            LoweredScalarEffect::EstablishByteSequenceLiteral {
                place,
                structural_type: _,
                bytes,
            } => {
                let id = operations.allocate();
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: OperationResult::Unit,
                    kind: OperationKind::EstablishByteSequenceLiteral {
                        destination: *place,
                        bytes: bytes.clone(),
                        qualifications: Vec::new(),
                    },
                });
            }
            LoweredScalarEffect::EstablishScalarArray(array) => {
                crate::scalar_graph::scalar_computations::arrays::emit(
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
            LoweredScalarEffect::ByteSequenceSubslice {
                source,
                access,
                start,
                end,
                place,
                structural_type,
            } => {
                view_subslice::emit(
                    view_subslice::ViewFamily::Bytes,
                    *source,
                    *access,
                    Some(start),
                    end.as_ref(),
                    *place,
                    *structural_type,
                    values,
                    next_value,
                    operations,
                )?;
            }
            LoweredScalarEffect::ElementViewSubslice {
                source,
                access,
                start,
                end,
                place,
                structural_type,
            } => {
                view_subslice::emit(
                    view_subslice::ViewFamily::Elements,
                    *source,
                    *access,
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
                let id = operations.allocate();
                operations.record_source_call_with_values(
                    call.source_coordinate,
                    None,
                    id,
                    call.target_state,
                    values,
                )?;
                let erased_proof_arguments = call
                    .erased_proof_arguments
                    .iter()
                    .map(|term| {
                        crate::scalar_graph::scalar_contracts::lowered_proof_term(
                            term, values, erased,
                        )
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id,
                    result: call
                        .result
                        .clone()
                        .map(OperationResult::Structural)
                        .unwrap_or(OperationResult::Unit),
                    kind: DirectCall {
                        callee: lookup_machine_id(calls.machine_ids, call.target_machine)?,
                        arguments: arguments.iter().map(|argument| argument.id).collect(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments,
                        structural_arguments: call.structural_arguments.clone(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: lower_checked_crash_route_buckets(
                            &call.crash_routes,
                            &arguments,
                        )?,
                        result: call
                            .result
                            .as_ref()
                            .map_or(DirectCallResult::Unit, |result| {
                                DirectCallResult::Structural {
                                    multiplicity: result.multiplicity,
                                    returned_claim_transfers: Vec::new(),
                                }
                            }),
                    }
                    .into_operation_kind(),
                });
            }
        }
    }
    Ok(())
}
